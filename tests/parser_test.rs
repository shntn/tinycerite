use tinycerilte::ast::{BinOp, Expr, Stmt};
use tinycerilte::parser::Parser;

fn parse(input: &str) -> tinycerilte::ast::Program {
    Parser::new(input)
        .parse_program()
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
    let mut p = Parser::new("{ var a: bit; a = }");
    assert!(p.parse_program().is_err());
}

#[test]
fn missing_rbrace_is_error() {
    let mut p = Parser::new("{ var a: bit; ");
    assert!(p.parse_program().is_err());
}