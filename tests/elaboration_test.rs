use tinycerilte::elaboration;
use tinycerilte::parser::Parser;

#[test]
fn elaborated_signals_have_correct_names_and_widths() {
    let prog = Parser::parse_program("testbench tb { var a: bit; var b: bit<16>; }")
        .unwrap();
    let elab = elaboration::elaborate(&prog).unwrap();
    assert_eq!(elab.top.signals.len(), 2);
    assert_eq!(elab.top.signals[0].name, "a");
    assert_eq!(elab.top.signals[0].width, 1);
    assert_eq!(elab.top.signals[1].name, "b");
    assert_eq!(elab.top.signals[1].width, 16);
}

#[test]
fn elaborated_stmts_preserve_assign_kind() {
    let prog = Parser::parse_program("testbench tb { var a: bit; var b: bit; a = b ^ 1; b <= a; }")
        .unwrap();
    let elab = elaboration::elaborate(&prog).unwrap();
    assert_eq!(elab.top.stmts.len(), 2);
    // combinatorial: target_id は a (id=0), sequential: target_id は b (id=1)
    assert!(matches!(elab.top.stmts[0], elaboration::ResolvedStmt::Combinational { target_id: 0, .. }));
    assert!(matches!(elab.top.stmts[1], elaboration::ResolvedStmt::Sequential { target_id: 1, .. }));
}

#[test]
fn undefined_signal_is_error() {
    let prog = Parser::parse_program("testbench tb { var a: bit; a = b ^ 1; }")
        .unwrap();
    let err = elaboration::elaborate(&prog).unwrap_err();
    assert!(err.message.contains("b"), "エラーメッセージに未定義変数名を含む");
}

#[test]
fn duplicate_declaration_is_error() {
    let prog = Parser::parse_program("testbench tb { var a: bit; var a: bit; }")
        .unwrap();
    let err = elaboration::elaborate(&prog).unwrap_err();
    assert!(err.message.contains("重複"), "エラーメッセージに重複を示す文言を含む");
}

#[test]
fn assignment_to_undeclared_target_is_error() {
    let prog = Parser::parse_program("testbench tb { x = 0; }")
        .unwrap();
    let err = elaboration::elaborate(&prog).unwrap_err();
    assert!(err.message.contains("x"));
}

#[test]
fn multiple_drivers_to_same_signal_is_error() {
    let prog = Parser::parse_program("testbench tb { var x: bit; x = 1; x = 0; }")
        .unwrap();
    let err = elaboration::elaborate(&prog).unwrap_err();
    assert!(err.message.contains("複数のドライバ"));
}

#[test]
fn combinational_self_loop_is_detected() {
    let prog = Parser::parse_program("testbench tb { var a: bit; a = a ^ 1; }")
        .unwrap();
    let err = elaboration::elaborate(&prog).unwrap_err();
    assert!(err.message.contains("組合せループ"), "エラー: {}", err.message);
}

#[test]
fn sequential_edge_does_not_cause_loop() {
    // sequential 代入を経由する循環は組合せループではない
    let prog = Parser::parse_program("testbench tb { var a: bit; var b: bit; a = b ^ 1; b <= a; }")
        .unwrap();
    assert!(elaboration::elaborate(&prog).is_ok(), "seq代入で切れるのでループなし");
}

#[test]
fn signal_declared_later_is_visible_to_earlier_stmt() {
    // 全宣言を先に集めてからstmtを解決するため、宣言より前に書かれたstmtからの前方参照が可能
    let prog = Parser::parse_program("testbench tb { a = b ^ 1; var a: bit; var b: bit; }").unwrap();
    assert!(
        elaboration::elaborate(&prog).is_ok(),
        "宣言より前に書かれたstmtからも前方参照できる"
    );
}

fn adder_src() -> &'static str {
    // ここでのテストはモジュール/インスタンス解決の仕組みが対象であり、reg(順序代入)の
    // clock要件とは無関係なため、組合せ代入で十分（clock入力ポート無しで書ける）
    "module adder { port { a: input bit<8>; b: input bit<8>; sum: output bit<8>; } sum = a + b; }"
}

#[test]
fn module_instantiation_resolves_successfully() {
    let src = format!(
        "{} testbench tb {{ var x: bit<8>; var y: bit<8>; var z: bit<8>; var u1 = adder(a: x, b: y); z = u1.sum; }}",
        adder_src()
    );
    let prog = Parser::parse_program(&src).unwrap();
    let elab = elaboration::elaborate(&prog).unwrap();
    assert_eq!(elab.modules.len(), 1);
    assert_eq!(elab.top.instances.len(), 1);
    assert_eq!(elab.top.instances[0].instance_name, "u1");
    assert_eq!(elab.top.instances[0].connections.len(), 2);
}

#[test]
fn duplicate_module_definition_is_error() {
    let src = format!("{} {}", adder_src(), adder_src());
    let prog = Parser::parse_program(&src).unwrap();
    let err = elaboration::elaborate(&prog).unwrap_err();
    assert!(err.message.contains("重複"), "エラー: {}", err.message);
}

#[test]
fn duplicate_port_name_is_error() {
    let prog = Parser::parse_program(
        "module m { port { a: input bit; a: output bit; } }",
    )
    .unwrap();
    let err = elaboration::elaborate(&prog).unwrap_err();
    assert!(err.message.contains("重複"), "エラー: {}", err.message);
}

#[test]
fn assigning_to_input_port_inside_module_is_error() {
    let prog = Parser::parse_program(
        "module m { port { a: input bit; b: output bit; } a = 1; b = a; }",
    )
    .unwrap();
    let err = elaboration::elaborate(&prog).unwrap_err();
    assert!(err.message.contains("入力ポート"), "エラー: {}", err.message);
}

#[test]
fn unused_module_body_is_still_validated() {
    // インスタンス化されなくても、モジュール定義自体は宣言時点で検証される
    let prog = Parser::parse_program(
        "module m { port { a: input bit; b: output bit; } b = c; }",
    )
    .unwrap();
    let err = elaboration::elaborate(&prog).unwrap_err();
    assert!(err.message.contains("c"), "エラー: {}", err.message);
}

#[test]
fn instantiating_undefined_module_is_error() {
    let prog = Parser::parse_program("testbench tb { var u1 = ghost(); }").unwrap();
    let err = elaboration::elaborate(&prog).unwrap_err();
    assert!(err.message.contains("ghost"), "エラー: {}", err.message);
}

#[test]
fn missing_input_connection_is_error() {
    let src = format!("{} testbench tb {{ var x: bit<8>; var u1 = adder(a: x); }}", adder_src());
    let prog = Parser::parse_program(&src).unwrap();
    let err = elaboration::elaborate(&prog).unwrap_err();
    assert!(err.message.contains("b"), "エラー: {}", err.message);
}

#[test]
fn connecting_output_port_as_argument_is_error() {
    let src = format!(
        "{} testbench tb {{ var x: bit<8>; var z: bit<8>; var u1 = adder(a: x, b: x, sum: z); }}",
        adder_src()
    );
    let prog = Parser::parse_program(&src).unwrap();
    let err = elaboration::elaborate(&prog).unwrap_err();
    assert!(err.message.contains("出力ポート"), "エラー: {}", err.message);
}

#[test]
fn unknown_port_name_as_argument_is_error() {
    let src = format!(
        "{} testbench tb {{ var x: bit<8>; var u1 = adder(a: x, b: x, ghost: x); }}",
        adder_src()
    );
    let prog = Parser::parse_program(&src).unwrap();
    let err = elaboration::elaborate(&prog).unwrap_err();
    assert!(err.message.contains("ghost"), "エラー: {}", err.message);
}

#[test]
fn instance_name_colliding_with_signal_name_is_error() {
    let src = format!(
        "{} testbench tb {{ var x: bit<8>; var x = adder(a: x, b: x); }}",
        adder_src()
    );
    let prog = Parser::parse_program(&src).unwrap();
    assert!(elaboration::elaborate(&prog).is_err());
}

#[test]
fn reading_input_port_field_from_outside_is_error() {
    let src = format!(
        "{} testbench tb {{ var x: bit<8>; var z: bit<8>; var u1 = adder(a: x, b: x); z = u1.a; }}",
        adder_src()
    );
    let prog = Parser::parse_program(&src).unwrap();
    let err = elaboration::elaborate(&prog).unwrap_err();
    assert!(err.message.contains("入力ポート"), "エラー: {}", err.message);
}

#[test]
fn reading_nonexistent_field_is_error() {
    let src = format!(
        "{} testbench tb {{ var x: bit<8>; var z: bit<8>; var u1 = adder(a: x, b: x); z = u1.ghost; }}",
        adder_src()
    );
    let prog = Parser::parse_program(&src).unwrap();
    let err = elaboration::elaborate(&prog).unwrap_err();
    assert!(err.message.contains("ghost"), "エラー: {}", err.message);
}

#[test]
fn instance_output_can_feed_another_instance_input() {
    // 同じスコープ内で、あるインスタンスの出力を別のインスタンスの入力に接続できる
    let src = format!(
        "{adder} testbench tb {{ var x: bit<8>; var z: bit<8>; var u1 = adder(a: x, b: x); var u2 = adder(a: u1.sum, b: x); z = u2.sum; }}",
        adder = adder_src()
    );
    let prog = Parser::parse_program(&src).unwrap();
    assert!(elaboration::elaborate(&prog).is_ok());
}

#[test]
fn testbench_initial_resolves_target_and_step_count() {
    let prog = Parser::parse_program(
        "testbench tb { var x: bit<8>; initial { x = 3; step; step; } }",
    )
    .unwrap();
    let elab = elaboration::elaborate(&prog).unwrap();
    assert_eq!(elab.initial.len(), 3);
    assert!(matches!(
        elab.initial[0],
        elaboration::ResolvedProcStmt::Assign { target_id: 0, .. }
    ));
    assert!(matches!(elab.initial[1], elaboration::ResolvedProcStmt::Step));
    assert!(matches!(elab.initial[2], elaboration::ResolvedProcStmt::Step));
}

#[test]
fn testbench_concurrent_signals_merge_into_top_level_scope() {
    let prog = Parser::parse_program("testbench tb { var clk: bit; clk <= !clk; }").unwrap();
    let elab = elaboration::elaborate(&prog).unwrap();
    assert_eq!(elab.top.signals.len(), 1);
    assert_eq!(elab.top.signals[0].name, "clk");
    assert_eq!(elab.top.stmts.len(), 1);
}

#[test]
fn testbench_instance_is_visible_to_initial_via_field_access() {
    let src = format!(
        "{adder} testbench tb {{ var x: bit<8>; var y: bit<8>; var z: bit<8>; var u1 = adder(a: x, b: y); initial {{ x = 3; y = 4; step; z = u1.sum; }} }}",
        adder = adder_src()
    );
    let prog = Parser::parse_program(&src).unwrap();
    assert!(elaboration::elaborate(&prog).is_ok());
}

#[test]
fn assigning_to_undeclared_signal_in_initial_is_error() {
    let prog = Parser::parse_program("testbench tb { initial { x = 3; } }").unwrap();
    let err = elaboration::elaborate(&prog).unwrap_err();
    assert!(err.message.contains("x"), "エラー: {}", err.message);
}

#[test]
fn multiple_testbenches_is_error() {
    let prog = Parser::parse_program("testbench a { } testbench b { }").unwrap();
    let err = elaboration::elaborate(&prog).unwrap_err();
    assert!(err.message.contains("1つ"), "エラー: {}", err.message);
}
