use tinycerilte::ast::{BinOp, Expr, Stmt};
use tinycerilte::parser::Parser;

fn parse(input: &str) -> tinycerilte::ast::Program {
    Parser::parse_program(input)
        .expect("パース成功を期待")
}

#[test]
fn empty_block() {
    let prog = parse("{}");
    assert_eq!(prog.blocks.len(), 1);
    assert_eq!(prog.blocks[0].decls.len(), 0);
    assert_eq!(prog.blocks[0].stmts.len(), 0);
}

#[test]
fn single_bit_declaration() {
    let prog = parse("{ var x: bit; }");
    let decl = &prog.blocks[0].decls[0];
    assert_eq!(decl.name, "x");
    assert_eq!(decl.width, None, "bit は幅なし = 1-bit");
}

#[test]
fn bit_vector_declaration() {
    let prog = parse("{ var x: bit<8>; }");
    let decl = &prog.blocks[0].decls[0];
    assert_eq!(decl.name, "x");
    assert_eq!(decl.width, Some(8));
}

#[test]
fn combinational_assign_is_blocking() {
    let prog = parse("{ a = b ^ 1; }");
    let stmt = &prog.blocks[0].stmts[0];
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
    let prog = parse("{ b <= a; }");
    let stmt = &prog.blocks[0].stmts[0];
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
    let input = "{\n    var a: bit;\n    var b: bit;\n\n    a = b ^ 1;\n    b <= a;\n}";
    let prog = parse(input);
    let block = &prog.blocks[0];
    assert_eq!(block.decls.len(), 2);
    assert_eq!(block.stmts.len(), 2);
    assert_eq!(block.decls[0].name, "a");
    assert_eq!(block.decls[1].name, "b");
    assert!(matches!(block.stmts[0], Stmt::Combinational { .. }));
    assert!(matches!(block.stmts[1], Stmt::Sequential { .. }));
}

#[test]
fn incomplete_statement_is_error() {
    assert!(Parser::parse_program("{ var a: bit; a = }").is_err());
}

#[test]
fn missing_rbrace_is_error() {
    assert!(Parser::parse_program("{ var a: bit; ").is_err());
}

#[test]
fn keyword_var_as_variable_name_is_error() {
    assert!(Parser::parse_program("{ var x: bit; var = 1; }").is_err());
}

#[test]
fn keyword_bit_as_variable_name_is_error() {
    assert!(Parser::parse_program("{ var x: bit; bit <= x; }").is_err());
}

#[test]
fn keyword_in_expression_is_error() {
    assert!(Parser::parse_program("{ var x: bit; x = var; }").is_err());
    assert!(Parser::parse_program("{ var x: bit; x = bit; }").is_err());
}

#[test]
fn chained_xor_with_three_or_more_operands_parses_successfully() {
    assert!(Parser::parse_program(
        "{ var a: bit; var b: bit; var c: bit; var d: bit; a = b ^ c ^ d; }"
    )
    .is_ok());
}

#[test]
fn multiple_top_level_blocks_are_parsed() {
    let prog = parse("{ var a: bit; } { var b: bit; }");
    assert_eq!(prog.blocks.len(), 2);
}

#[test]
fn decl_and_stmt_can_interleave_in_any_order() {
    let prog = parse("{ var a: bit; a = 1; var b: bit; b <= a; }");
    assert_eq!((prog.blocks[0].decls.len(), prog.blocks[0].stmts.len()), (2, 2));
}

#[test]
fn leading_zeros_in_number_literal_are_parsed() {
    let prog = parse("{ var x: bit; x = 007; }");
    let stmt = &prog.blocks[0].stmts[0];
    match stmt {
        Stmt::Combinational { expr, .. } => assert!(matches!(expr, Expr::Number(7))),
        _ => panic!("comb assign が期待される"),
    }
}

#[test]
fn overflow_number_literal_is_error() {
    assert!(Parser::parse_program("{ var x: bit; x = 99999999999999999999; }").is_err());
}

#[test]
fn empty_input_is_error() {
    assert!(Parser::parse_program("").is_err());
}

#[test]
fn multiplication_binds_tighter_than_addition() {
    let prog = parse("{ var x: bit; x = 1 + 2 * 3; }");
    let stmt = &prog.blocks[0].stmts[0];
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
    assert!(Parser::parse_program("{ var x: bit; x = (1 + 2) * 3; }").is_ok());
}

#[test]
fn all_new_binary_operators_parse_successfully() {
    let ops = [
        "||", "&&", "|", "&", "==", "!=", "<", "<=", ">", ">=", "<<", ">>", "<<<", ">>>", "+", "-",
        "*", "/", "%",
    ];
    for op in ops {
        let src = format!("{{ var x: bit; x = 1 {op} 2; }}");
        assert!(Parser::parse_program(&src).is_ok(), "演算子 {op} がパースできる");
    }
}
