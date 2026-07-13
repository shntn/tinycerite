use tinycerilte::elaboration;
use tinycerilte::parser::Parser;

#[test]
fn elaborated_signals_have_correct_names_and_widths() {
    let prog = Parser::parse_program("{ var a: bit; var b: bit<16>; }")
        .unwrap();
    let elab = elaboration::elaborate(&prog).unwrap();
    assert_eq!(elab.signals.len(), 2);
    assert_eq!(elab.signals[0].name, "a");
    assert_eq!(elab.signals[0].width, 1);
    assert_eq!(elab.signals[1].name, "b");
    assert_eq!(elab.signals[1].width, 16);
}

#[test]
fn elaborated_stmts_preserve_assign_kind() {
    let prog = Parser::parse_program("{ var a: bit; var b: bit; a = b ^ 1; b <= a; }")
        .unwrap();
    let elab = elaboration::elaborate(&prog).unwrap();
    assert_eq!(elab.stmts.len(), 2);
    // combinatorial: target_id は a (id=0), sequential: target_id は b (id=1)
    assert!(matches!(elab.stmts[0], elaboration::ResolvedStmt::Combinational { target_id: 0, .. }));
    assert!(matches!(elab.stmts[1], elaboration::ResolvedStmt::Sequential { target_id: 1, .. }));
}

#[test]
fn undefined_signal_is_error() {
    let prog = Parser::parse_program("{ var a: bit; a = b ^ 1; }")
        .unwrap();
    let err = elaboration::elaborate(&prog).unwrap_err();
    assert!(err.message.contains("b"), "エラーメッセージに未定義変数名を含む");
}

#[test]
fn duplicate_declaration_is_error() {
    let prog = Parser::parse_program("{ var a: bit; var a: bit; }")
        .unwrap();
    let err = elaboration::elaborate(&prog).unwrap_err();
    assert!(err.message.contains("重複"), "エラーメッセージに重複を示す文言を含む");
}

#[test]
fn assignment_to_undeclared_target_is_error() {
    let prog = Parser::parse_program("{ x = 0; }")
        .unwrap();
    let err = elaboration::elaborate(&prog).unwrap_err();
    assert!(err.message.contains("x"));
}

#[test]
fn multiple_drivers_to_same_signal_is_error() {
    let prog = Parser::parse_program("{ var x: bit; x = 1; x = 0; }")
        .unwrap();
    let err = elaboration::elaborate(&prog).unwrap_err();
    assert!(err.message.contains("複数のドライバ"));
}

#[test]
fn combinational_self_loop_is_detected() {
    let prog = Parser::parse_program("{ var a: bit; a = a ^ 1; }")
        .unwrap();
    let err = elaboration::elaborate(&prog).unwrap_err();
    assert!(err.message.contains("組合せループ"), "エラー: {}", err.message);
}

#[test]
fn sequential_edge_does_not_cause_loop() {
    // sequential 代入を経由する循環は組合せループではない
    let prog = Parser::parse_program("{ var a: bit; var b: bit; a = b ^ 1; b <= a; }")
        .unwrap();
    assert!(elaboration::elaborate(&prog).is_ok(), "seq代入で切れるのでループなし");
}

#[test]
fn signal_declared_in_later_block_is_visible_to_earlier_block_stmt() {
    // 全ブロックの宣言を先に集めてからstmtを解決するため、ブロックをまたいだ前方参照が可能
    let prog = Parser::parse_program("{ a = b ^ 1; } { var a: bit; var b: bit; }").unwrap();
    assert!(
        elaboration::elaborate(&prog).is_ok(),
        "ブロックをまたいでも名前空間はフラットに共有される"
    );
}
