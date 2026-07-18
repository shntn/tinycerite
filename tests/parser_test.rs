use tinycerilte::ast::{BinOp, Direction, Expr, ProcStmt, SignalType, Stmt, UnOp};
use tinycerilte::parser::Parser;

fn parse(input: &str) -> tinycerilte::ast::Program {
    Parser::parse_program(input)
        .expect("パース成功を期待")
}

#[test]
fn empty_testbench() {
    let prog = parse("testbench tb {}");
    assert_eq!(prog.testbenches.len(), 1);
    assert_eq!(prog.testbenches[0].decls.len(), 0);
    assert_eq!(prog.testbenches[0].stmts.len(), 0);
}

#[test]
fn single_bit_declaration() {
    let prog = parse("testbench tb { var x: bit; }");
    let decl = &prog.testbenches[0].decls[0];
    assert_eq!(decl.name, "x");
    assert_eq!(decl.sig_type, SignalType::Bit(None), "bit は幅なし = 1-bit");
}

#[test]
fn bit_vector_declaration() {
    let prog = parse("testbench tb { var x: bit<8>; }");
    let decl = &prog.testbenches[0].decls[0];
    assert_eq!(decl.name, "x");
    assert_eq!(decl.sig_type, SignalType::Bit(Some(8)));
}

#[test]
fn combinational_assign_is_blocking() {
    let prog = parse("testbench tb { a = b ^ 1; }");
    let stmt = &prog.testbenches[0].stmts[0];
    match stmt {
        Stmt::Combinational { target, expr } => {
            assert_eq!(target, "a");
            assert!(matches!(expr, Expr::BinOp { op: BinOp::Xor, .. }));
        }
        _ => panic!("= は Combinational としてパースされるべき"),
    }
}

#[test]
fn sequential_assign_is_non_blocking() {
    let prog = parse("testbench tb { b <= a; }");
    let stmt = &prog.testbenches[0].stmts[0];
    match stmt {
        Stmt::Sequential { target, expr } => {
            assert_eq!(target, "b");
            assert!(matches!(expr, Expr::Ident(name) if name == "a"));
        }
        _ => panic!("<= は Sequential としてパースされるべき"),
    }
}

#[test]
fn full_example_parses_correctly() {
    let input = "testbench tb {\n    var a: bit;\n    var b: bit;\n\n    a = b ^ 1;\n    b <= a;\n}";
    let prog = parse(input);
    let tb = &prog.testbenches[0];
    assert_eq!(tb.decls.len(), 2);
    assert_eq!(tb.stmts.len(), 2);
    assert_eq!(tb.decls[0].name, "a");
    assert_eq!(tb.decls[1].name, "b");
    assert!(matches!(tb.stmts[0], Stmt::Combinational { .. }));
    assert!(matches!(tb.stmts[1], Stmt::Sequential { .. }));
}

#[test]
fn incomplete_statement_is_error() {
    assert!(Parser::parse_program("testbench tb { var a: bit; a = }").is_err());
}

#[test]
fn missing_rbrace_is_error() {
    assert!(Parser::parse_program("testbench tb { var a: bit; ").is_err());
}

#[test]
fn keyword_var_as_variable_name_is_error() {
    assert!(Parser::parse_program("testbench tb { var x: bit; var = 1; }").is_err());
}

#[test]
fn keyword_bit_as_variable_name_is_error() {
    assert!(Parser::parse_program("testbench tb { var x: bit; bit <= x; }").is_err());
}

#[test]
fn keyword_in_expression_is_error() {
    assert!(Parser::parse_program("testbench tb { var x: bit; x = var; }").is_err());
    assert!(Parser::parse_program("testbench tb { var x: bit; x = bit; }").is_err());
}

#[test]
fn chained_xor_with_three_or_more_operands_parses_successfully() {
    assert!(Parser::parse_program(
        "testbench tb { var a: bit; var b: bit; var c: bit; var d: bit; a = b ^ c ^ d; }"
    )
    .is_ok());
}

#[test]
fn decl_and_stmt_can_interleave_in_any_order() {
    let prog = parse("testbench tb { var a: bit; a = 1; var b: bit; b <= a; }");
    let tb = &prog.testbenches[0];
    assert_eq!((tb.decls.len(), tb.stmts.len()), (2, 2));
}

#[test]
fn leading_zeros_in_number_literal_are_parsed() {
    let prog = parse("testbench tb { var x: bit; x = 007; }");
    let stmt = &prog.testbenches[0].stmts[0];
    match stmt {
        Stmt::Combinational { expr, .. } => assert!(matches!(expr, Expr::Number(7))),
        _ => panic!("comb assign が期待される"),
    }
}

#[test]
fn overflow_number_literal_is_error() {
    assert!(Parser::parse_program("testbench tb { var x: bit; x = 99999999999999999999; }").is_err());
}

#[test]
fn empty_input_is_error() {
    assert!(Parser::parse_program("").is_err());
}

#[test]
fn multiplication_binds_tighter_than_addition() {
    let prog = parse("testbench tb { var x: bit; x = 1 + 2 * 3; }");
    let stmt = &prog.testbenches[0].stmts[0];
    match stmt {
        Stmt::Combinational { expr, .. } => {
            // 1 + (2 * 3) の形（Addが最外、rhsがMulのBinOp）になっているはず
            match expr {
                Expr::BinOp { op: BinOp::Add, rhs, .. } => {
                    assert!(matches!(**rhs, Expr::BinOp { op: BinOp::Mul, .. }));
                }
                _ => panic!("最外は Add であるべき"),
            }
        }
        _ => panic!("comb assign が期待される"),
    }
}

#[test]
fn parenthesized_expression_overrides_precedence() {
    assert!(Parser::parse_program("testbench tb { var x: bit; x = (1 + 2) * 3; }").is_ok());
}

#[test]
fn all_new_binary_operators_parse_successfully() {
    let ops = [
        "||", "&&", "|", "&", "==", "!=", "<", "<=", ">", ">=", "<<", ">>", "<<<", ">>>", "+", "-",
        "*", "/", "%",
    ];
    for op in ops {
        let src = format!("testbench tb {{ var x: bit; x = 1 {op} 2; }}");
        assert!(Parser::parse_program(&src).is_ok(), "演算子 {op} がパースできる");
    }
}

#[test]
fn unary_not_parses_as_unaryop() {
    let prog = parse("testbench tb { var x: bit; x = !x; }");
    let stmt = &prog.testbenches[0].stmts[0];
    match stmt {
        Stmt::Combinational { expr, .. } => {
            assert!(matches!(expr, Expr::UnaryOp { op: UnOp::Not, .. }));
        }
        _ => panic!("comb assign が期待される"),
    }
}

#[test]
fn unary_bitnot_parses_as_unaryop() {
    let prog = parse("testbench tb { var x: bit; x = ~x; }");
    let stmt = &prog.testbenches[0].stmts[0];
    match stmt {
        Stmt::Combinational { expr, .. } => {
            assert!(matches!(expr, Expr::UnaryOp { op: UnOp::BitNot, .. }));
        }
        _ => panic!("comb assign が期待される"),
    }
}

#[test]
fn chained_unary_operators_nest_right_to_left() {
    let prog = parse("testbench tb { var x: bit; x = !~x; }");
    let stmt = &prog.testbenches[0].stmts[0];
    match stmt {
        Stmt::Combinational { expr, .. } => match expr {
            Expr::UnaryOp { op: UnOp::Not, expr } => {
                assert!(matches!(**expr, Expr::UnaryOp { op: UnOp::BitNot, .. }));
            }
            _ => panic!("最外は Not(BitNot(x)) であるべき"),
        },
        _ => panic!("comb assign が期待される"),
    }
}

#[test]
fn unary_operator_binds_tighter_than_multiplication() {
    let prog = parse("testbench tb { var x: bit; x = !x * 2; }");
    let stmt = &prog.testbenches[0].stmts[0];
    match stmt {
        Stmt::Combinational { expr, .. } => {
            // (!x) * 2 の形（Mulが最外、lhsがUnaryOp）になっているはず
            match expr {
                Expr::BinOp { op: BinOp::Mul, lhs, .. } => {
                    assert!(matches!(**lhs, Expr::UnaryOp { op: UnOp::Not, .. }));
                }
                _ => panic!("最外は Mul であるべき"),
            }
        }
        _ => panic!("comb assign が期待される"),
    }
}

#[test]
fn binary_bitvec_literal_parses_correctly() {
    let prog = parse("testbench tb { var x: bit<4>; x = 4'b1010; }");
    let stmt = &prog.testbenches[0].stmts[0];
    match stmt {
        Stmt::Combinational { expr, .. } => {
            assert!(matches!(expr, Expr::BitVecLiteral { width: 4, value: 10 }));
        }
        _ => panic!("comb assign が期待される"),
    }
}

#[test]
fn hex_bitvec_literal_parses_correctly() {
    let prog = parse("testbench tb { var x: bit<8>; x = 8'hFF; }");
    let stmt = &prog.testbenches[0].stmts[0];
    match stmt {
        Stmt::Combinational { expr, .. } => {
            assert!(matches!(expr, Expr::BitVecLiteral { width: 8, value: 255 }));
        }
        _ => panic!("comb assign が期待される"),
    }
}

#[test]
fn octal_and_decimal_bitvec_literals_parse_correctly() {
    let prog = parse("testbench tb { var x: bit<8>; var y: bit<8>; x = 8'o17; y = 8'd200; }");
    match &prog.testbenches[0].stmts[0] {
        Stmt::Combinational { expr, .. } => {
            assert!(matches!(expr, Expr::BitVecLiteral { width: 8, value: 15 }));
        }
        _ => panic!("comb assign が期待される"),
    }
    match &prog.testbenches[0].stmts[1] {
        Stmt::Combinational { expr, .. } => {
            assert!(matches!(expr, Expr::BitVecLiteral { width: 8, value: 200 }));
        }
        _ => panic!("comb assign が期待される"),
    }
}

#[test]
fn lowercase_hex_digits_in_bitvec_literal_parse_correctly() {
    let prog = parse("testbench tb { var x: bit<8>; x = 8'hff; }");
    let stmt = &prog.testbenches[0].stmts[0];
    match stmt {
        Stmt::Combinational { expr, .. } => {
            assert!(matches!(expr, Expr::BitVecLiteral { width: 8, value: 255 }));
        }
        _ => panic!("comb assign が期待される"),
    }
}

#[test]
fn digit_invalid_for_radix_in_bitvec_literal_is_error() {
    assert!(Parser::parse_program("testbench tb { var x: bit<4>; x = 2'b19; }").is_err());
}

#[test]
fn ternary_operator_parses_as_ternary_expr() {
    let prog = parse("testbench tb { var x: bit; var a: bit; x = a ? 1 : 0; }");
    let stmt = &prog.testbenches[0].stmts[0];
    match stmt {
        Stmt::Combinational { expr, .. } => {
            assert!(matches!(expr, Expr::Ternary { .. }));
        }
        _ => panic!("comb assign が期待される"),
    }
}

#[test]
fn ternary_operator_is_right_associative() {
    // a ? b : c ? d : e は a ? b : (c ? d : e) になるはず
    let prog = parse("testbench tb { var x: bit; var a: bit; var c: bit; x = a ? 1 : c ? 2 : 3; }");
    let stmt = &prog.testbenches[0].stmts[0];
    match stmt {
        Stmt::Combinational { expr, .. } => match expr {
            Expr::Ternary { else_branch, .. } => {
                assert!(matches!(**else_branch, Expr::Ternary { .. }));
            }
            _ => panic!("最外は Ternary であるべき"),
        },
        _ => panic!("comb assign が期待される"),
    }
}

#[test]
fn ternary_operator_has_lower_precedence_than_binary_operators() {
    // a || b ? 1 : 0 は (a || b) ? 1 : 0 になるはず（condが二項式全体）
    let prog = parse("testbench tb { var x: bit; var a: bit; var b: bit; x = a || b ? 1 : 0; }");
    let stmt = &prog.testbenches[0].stmts[0];
    match stmt {
        Stmt::Combinational { expr, .. } => match expr {
            Expr::Ternary { cond, .. } => {
                assert!(matches!(**cond, Expr::BinOp { op: BinOp::Or, .. }));
            }
            _ => panic!("最外は Ternary であるべき"),
        },
        _ => panic!("comb assign が期待される"),
    }
}

#[test]
fn parenthesized_ternary_expression_parses_successfully() {
    assert!(Parser::parse_program("testbench tb { var x: bit; var a: bit; x = (a ? 1 : 0) + 1; }").is_ok());
}

#[test]
fn not_equal_operator_is_unaffected_by_unary_not() {
    // "!=" は eq_op として扱われ、単項の "!" とは別物であることを確認
    let prog = parse("testbench tb { var x: bit; var y: bit; x = y != 1; }");
    let stmt = &prog.testbenches[0].stmts[0];
    match stmt {
        Stmt::Combinational { expr, .. } => {
            assert!(matches!(expr, Expr::BinOp { op: BinOp::Neq, .. }));
        }
        _ => panic!("comb assign が期待される"),
    }
}

#[test]
fn module_def_parses_ports_and_body() {
    let prog = parse(
        "module adder { port { a: input bit<8>; b: input bit<8>; sum: output bit<8>; } sum <= a + b; }",
    );
    assert_eq!(prog.modules.len(), 1);
    let m = &prog.modules[0];
    assert_eq!(m.name, "adder");
    assert_eq!(m.ports.len(), 3);
    assert_eq!(m.ports[0].name, "a");
    assert_eq!(m.ports[0].direction, Direction::Input);
    assert_eq!(m.ports[0].sig_type, SignalType::Bit(Some(8)));
    assert_eq!(m.ports[2].name, "sum");
    assert_eq!(m.ports[2].direction, Direction::Output);
    assert_eq!(m.stmts.len(), 1);
}

#[test]
fn module_def_can_have_internal_decls() {
    let prog = parse(
        "module m { port { a: input bit; } var internal: bit; internal = a; }",
    );
    let m = &prog.modules[0];
    assert_eq!(m.decls.len(), 1);
    assert_eq!(m.decls[0].name, "internal");
}

#[test]
fn multiple_modules_and_testbench_can_coexist() {
    let prog = parse(
        "module a { port {} } testbench tb { var x: bit; } module b { port {} }",
    );
    assert_eq!(prog.modules.len(), 2);
    assert_eq!(prog.testbenches.len(), 1);
}

#[test]
fn instance_decl_parses_with_named_args() {
    let prog = parse(
        "testbench tb { var x: bit<8>; var y: bit<8>; var u1 = adder(a: x, b: y); }",
    );
    let inst = &prog.testbenches[0].instances[0];
    assert_eq!(inst.instance_name, "u1");
    assert_eq!(inst.module_name, "adder");
    assert_eq!(inst.args.len(), 2);
    assert_eq!(inst.args[0].0, "a");
    assert!(matches!(inst.args[0].1, Expr::Ident(ref name) if name == "x"));
}

#[test]
fn instance_decl_with_no_args_parses() {
    let prog = parse("testbench tb { var u1 = m(); }");
    assert_eq!(prog.testbenches[0].instances[0].args.len(), 0);
}

#[test]
fn field_access_parses_as_expr() {
    let prog = parse("testbench tb { var z: bit<8>; var u1 = adder(); z = u1.sum; }");
    let stmt = &prog.testbenches[0].stmts[0];
    match stmt {
        Stmt::Combinational { expr, .. } => {
            assert!(matches!(expr, Expr::FieldAccess { instance, field } if instance == "u1" && field == "sum"));
        }
        _ => panic!("comb assign が期待される"),
    }
}

#[test]
fn field_access_usable_inside_larger_expression() {
    assert!(Parser::parse_program("testbench tb { var z: bit<8>; var u1 = adder(); z = u1.sum + 1; }").is_ok());
}

#[test]
fn module_keyword_as_module_name_is_error() {
    assert!(Parser::parse_program("module module { port {} }").is_err());
}

#[test]
fn input_keyword_as_port_name_is_error() {
    assert!(Parser::parse_program("module m { port { input: input bit; } }").is_err());
}

#[test]
fn output_keyword_as_signal_name_is_error() {
    assert!(Parser::parse_program("testbench tb { var output: bit; }").is_err());
}

#[test]
fn port_without_direction_is_error() {
    assert!(Parser::parse_program("module m { port { a: bit; } }").is_err());
}

#[test]
fn module_cannot_contain_instance_decl() {
    // モジュール本体はネストしたインスタンス化を許可しない（文法レベルで弾く）
    assert!(Parser::parse_program(
        "module m { port {} var u1 = other(); }"
    )
    .is_err());
}

#[test]
fn line_comment_is_ignored_between_statements() {
    let prog = parse("testbench tb { var x: bit; // これはコメント\n x = 1; }");
    assert_eq!(prog.testbenches[0].stmts.len(), 1);
}

#[test]
fn line_comment_at_end_of_line_is_ignored() {
    let prog = parse("testbench tb { var x: bit; x = 1; // 行末コメント\n }");
    assert_eq!(prog.testbenches[0].stmts.len(), 1);
}

#[test]
fn trailing_unparseable_garbage_after_valid_program_is_error() {
    // programルールがEOIまで消費することを要求していないと、末尾の不正な入力が
    // 静かに無視されてしまう（過去に実際に発生したバグ）。EOIの強制でエラーになることを確認する。
    assert!(Parser::parse_program("testbench tb { var x: bit; } @@@invalid@@@").is_err());
}

#[test]
fn testbench_def_parses_concurrent_and_initial_parts() {
    let prog = parse(
        "testbench tb { var clk: bit; clk <= !clk; initial { clk = 0; step; step; } }",
    );
    assert_eq!(prog.testbenches.len(), 1);
    let tb = &prog.testbenches[0];
    assert_eq!(tb.name, "tb");
    assert_eq!(tb.decls.len(), 1);
    assert_eq!(tb.stmts.len(), 1);
    assert_eq!(tb.initial.len(), 3);
}

#[test]
fn testbench_can_have_instance_decl() {
    let prog = parse(
        "testbench tb { var x: bit<8>; var y: bit<8>; var u1 = adder(a: x, b: y); }",
    );
    assert_eq!(prog.testbenches[0].instances.len(), 1);
}

#[test]
fn testbench_without_initial_block_parses() {
    let prog = parse("testbench tb { var clk: bit; clk <= !clk; }");
    assert_eq!(prog.testbenches[0].initial.len(), 0);
}

#[test]
fn initial_proc_assign_parses_as_assign_variant() {
    let prog = parse("testbench tb { var x: bit<8>; initial { x = 3; } }");
    match &prog.testbenches[0].initial[0] {
        ProcStmt::Assign { target, expr } => {
            assert_eq!(target, "x");
            assert!(matches!(expr, Expr::Number(3)));
        }
        _ => panic!("Assignが期待される"),
    }
}

#[test]
fn initial_step_parses_as_step_variant() {
    let prog = parse("testbench tb { initial { step; } }");
    assert!(matches!(prog.testbenches[0].initial[0], ProcStmt::Step));
}

#[test]
fn testbench_keyword_as_name_is_error() {
    assert!(Parser::parse_program("testbench testbench { }").is_err());
}

#[test]
fn initial_keyword_as_signal_name_is_error() {
    assert!(Parser::parse_program("testbench tb { var initial: bit; }").is_err());
}

#[test]
fn step_keyword_as_signal_name_is_error() {
    assert!(Parser::parse_program("testbench tb { var step: bit; }").is_err());
}

#[test]
fn multiple_testbenches_parse_but_are_rejected_later() {
    // 文法上は複数書けてしまうが、「1つだけ」の制約はエラボレーションで検証する
    let prog = parse("testbench a { } testbench b { }");
    assert_eq!(prog.testbenches.len(), 2);
}

#[test]
fn if_elif_else_desugars_to_nested_ternary_combinational_stmt() {
    let prog = parse(
        "testbench tb { var a: bit<8>; var x: bit<8>; if a == 0 { x = 10; } elif a == 1 { x = 20; } else { x = 30; } }",
    );
    let stmts = &prog.testbenches[0].stmts;
    assert_eq!(stmts.len(), 1, "if/elif/elseは変数xへの代入1文だけに脱糖される");
    match &stmts[0] {
        Stmt::Combinational { target, expr } => {
            assert_eq!(target, "x");
            match expr {
                Expr::Ternary { cond, then_branch, else_branch } => {
                    assert!(matches!(**cond, Expr::BinOp { op: BinOp::Eq, .. }), "最外はifの条件(a==0)");
                    assert!(matches!(**then_branch, Expr::Number(10)));
                    assert!(matches!(**else_branch, Expr::Ternary { .. }), "elseの位置にelifが入れ子になる");
                }
                _ => panic!("Ternaryが期待される"),
            }
        }
        _ => panic!("Combinationalが期待される"),
    }
}

#[test]
fn if_else_without_elif_desugars_correctly() {
    let prog = parse("testbench tb { var a: bit; var x: bit; if a == 0 { x = 1; } else { x = 0; } }");
    assert_eq!(prog.testbenches[0].stmts.len(), 1);
}

#[test]
fn if_without_else_is_error() {
    assert!(Parser::parse_program("testbench tb { var a: bit; var x: bit; if a == 0 { x = 1; } }").is_err());
}

#[test]
fn if_branches_with_mismatched_targets_is_error() {
    let prog = Parser::parse_program(
        "testbench tb { var a: bit; var x: bit; var y: bit; if a == 0 { x = 1; y = 1; } else { x = 0; } }",
    );
    assert!(prog.is_err(), "elseにyへの代入が無いのでエラー");
}

#[test]
fn if_branches_with_mismatched_assign_kind_is_error() {
    let prog = Parser::parse_program(
        "testbench tb { var a: bit; var x: bit; if a == 0 { x = 1; } else { x <= 0; } }",
    );
    assert!(prog.is_err(), "同じ変数への代入演算子(=/<=)が分岐によって違うのでエラー");
}

#[test]
fn if_branch_assigning_same_target_twice_is_error() {
    let prog = Parser::parse_program(
        "testbench tb { var a: bit; var x: bit; if a == 0 { x = 1; x = 2; } else { x = 0; } }",
    );
    assert!(prog.is_err(), "同じ分岐内で同じ変数に2回代入するのはエラー");
}

#[test]
fn if_preserves_sequential_assignment_kind() {
    let prog = parse("testbench tb { var a: bit; var x: bit; if a == 0 { x <= 1; } else { x <= 0; } }");
    match &prog.testbenches[0].stmts[0] {
        Stmt::Sequential { target, .. } => assert_eq!(target, "x"),
        _ => panic!("Sequentialが期待される"),
    }
}

#[test]
fn if_can_assign_multiple_targets_per_branch() {
    let prog = parse(
        "testbench tb { var a: bit; var x: bit; var y: bit; if a == 0 { x = 1; y = 2; } else { x = 3; y = 4; } }",
    );
    assert_eq!(prog.testbenches[0].stmts.len(), 2, "x, yそれぞれに1文ずつ脱糖される");
}

#[test]
fn nested_if_inside_else_branch_parses_and_flattens() {
    let prog = parse(
        "testbench tb { var a: bit; var b: bit; var y: bit; \
         if a == 0 { y = 1; } else { if b == 0 { y = 2; } else { y = 3; } } }",
    );
    assert_eq!(prog.testbenches[0].stmts.len(), 1, "入れ子のif/elseも最終的に1文へ脱糖される");
}

#[test]
fn if_keyword_as_signal_name_is_error() {
    assert!(Parser::parse_program("testbench tb { var if: bit; }").is_err());
}

#[test]
fn elif_keyword_as_signal_name_is_error() {
    assert!(Parser::parse_program("testbench tb { var elif: bit; }").is_err());
}

#[test]
fn else_keyword_as_signal_name_is_error() {
    assert!(Parser::parse_program("testbench tb { var else: bit; }").is_err());
}
