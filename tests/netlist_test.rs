use tinycerilte::elaboration;
use tinycerilte::netlist::{self, DriveKind, InitialStep, SignalKind};
use tinycerilte::parser::Parser;

fn netlist_of(input: &str) -> netlist::Netlist {
    let prog = Parser::parse_program(input).unwrap();
    let elab = elaboration::elaborate(&prog).unwrap();
    netlist::build_netlist(&elab)
}

fn whole_pipeline(input: &str) -> String {
    let prog = Parser::parse_program(input).unwrap();
    let elab = elaboration::elaborate(&prog).unwrap();
    let nl = netlist::build_netlist(&elab);
    netlist::format_netlist(&nl)
}

#[test]
fn example1_produces_correct_signal_count() {
    let input = "{\n    var     a: bit;\n    var     b: bit;\n\n    a = b ^ 1;\n    b <= a;\n}";
    let nl = netlist_of(input);
    assert_eq!(nl.signals.len(), 2);
}

#[test]
fn example1_produces_six_nodes() {
    let input = "{\n    var     a: bit;\n    var     b: bit;\n\n    a = b ^ 1;\n    b <= a;\n}";
    let nl = netlist_of(input);
    // Const(1), Read(b), Xor, Drive(a), Read(a), Drive(b)
    assert_eq!(nl.nodes.len(), 6);
}

#[test]
fn blocking_assign_is_combinational() {
    let input = "{ var a: bit; var b: bit; a = b ^ 1; }";
    let nl = netlist_of(input);
    // a のドライバは blocking
    let sig = &nl.signals[0];
    assert_eq!(sig.name, "a");
    assert_eq!(sig.driver_kind, Some(DriveKind::Combinational));
}

#[test]
fn non_blocking_assign_is_sequential() {
    let input = "{ var a: bit; var b: bit; b <= a; }";
    let nl = netlist_of(input);
    // b のドライバは non-blocking
    let sig = &nl.signals[1];
    assert_eq!(sig.name, "b");
    assert_eq!(sig.driver_kind, Some(DriveKind::Sequential));
}

#[test]
fn format_output_contains_signal_and_node_sections() {
    let text = whole_pipeline("{ var a: bit; a = 0; }");
    assert!(text.contains("Signals"), "出力に Signals セクションが必要");
    assert!(text.contains("Nodes"), "出力に Nodes セクションが必要");
}

#[test]
fn bit_vector_width_is_preserved() {
    let input = "{ var x: bit<8>; x = 0; }";
    let nl = netlist_of(input);
    assert_eq!(nl.signals[0].width, 8);
}

#[test]
fn bit_width_one_by_default() {
    let input = "{ var x: bit; x = 0; }";
    let nl = netlist_of(input);
    assert_eq!(nl.signals[0].width, 1);
}

#[test]
fn combinational_signal_defaults_to_wire_kind() {
    let input = "{ var a: bit; var b: bit; a = b ^ 1; }";
    let nl = netlist_of(input);
    assert_eq!(nl.signals[0].kind, SignalKind::Wire);
}

#[test]
fn sequential_signal_defaults_to_reg_kind_with_no_clock_or_reset() {
    let input = "{ var a: bit; var b: bit; b <= a; }";
    let nl = netlist_of(input);
    assert_eq!(
        nl.signals[1].kind,
        SignalKind::Reg { clock: None, reset: None },
        "クロック/リセット未指定は現行のstep単位更新のまま"
    );
}

#[test]
fn undriven_signal_defaults_to_wire_kind() {
    let input = "{ var a: bit; }";
    let nl = netlist_of(input);
    assert_eq!(nl.signals[0].kind, SignalKind::Wire);
}

fn adder_src() -> &'static str {
    "module adder { port { a: input bit<8>; b: input bit<8>; sum: output bit<8>; } sum = a + b; }"
}

#[test]
fn module_instance_signals_are_flattened_with_instance_name_prefix() {
    let src = format!(
        "{} {{ var x: bit<8>; var y: bit<8>; var u1 = adder(a: x, b: y); }}",
        adder_src()
    );
    let nl = netlist_of(&src);
    let names: Vec<&str> = nl.signals.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"x"));
    assert!(names.contains(&"y"));
    assert!(names.contains(&"u1.a"));
    assert!(names.contains(&"u1.b"));
    assert!(names.contains(&"u1.sum"));
}

#[test]
fn module_instance_signals_preserve_declared_width() {
    let src = format!(
        "{} {{ var x: bit<8>; var y: bit<8>; var u1 = adder(a: x, b: y); }}",
        adder_src()
    );
    let nl = netlist_of(&src);
    let sum = nl.signals.iter().find(|s| s.name == "u1.sum").unwrap();
    assert_eq!(sum.width, 8);
}

#[test]
fn module_instance_input_port_is_driven_by_connection_expr() {
    let src = format!(
        "{} {{ var x: bit<8>; var y: bit<8>; var u1 = adder(a: x, b: y); }}",
        adder_src()
    );
    let nl = netlist_of(&src);
    let a = nl.signals.iter().find(|s| s.name == "u1.a").unwrap();
    assert_eq!(a.driver_kind, Some(DriveKind::Combinational), "u1.aはx接続の合成Driveで駆動される");
}

#[test]
fn two_instances_of_the_same_module_get_distinct_namespaces() {
    let src = format!(
        "{adder} {{ var x: bit<8>; var u1 = adder(a: x, b: x); var u2 = adder(a: x, b: x); }}",
        adder = adder_src()
    );
    let nl = netlist_of(&src);
    let names: Vec<&str> = nl.signals.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"u1.sum"));
    assert!(names.contains(&"u2.sum"));
    assert_eq!(nl.signals.iter().filter(|s| s.name.ends_with(".sum")).count(), 2);
}

#[test]
fn no_initial_block_produces_empty_initial_steps() {
    let nl = netlist_of("{ var a: bit; a = 0; }");
    assert!(nl.initial.is_empty());
}

#[test]
fn initial_block_produces_matching_step_sequence() {
    let nl = netlist_of("testbench tb { var x: bit<8>; initial { x = 3; step; step; } }");
    assert_eq!(nl.initial.len(), 3);
    assert!(matches!(nl.initial[0], InitialStep::Assign { .. }));
    assert!(matches!(nl.initial[1], InitialStep::Step));
    assert!(matches!(nl.initial[2], InitialStep::Step));
}

#[test]
fn initial_assign_targets_correct_global_signal_id() {
    let nl = netlist_of("testbench tb { var a: bit<8>; var x: bit<8>; initial { x = 3; } }");
    let x_id = nl.signals.iter().position(|s| s.name == "x").unwrap();
    match nl.initial[0] {
        InitialStep::Assign { target, .. } => assert_eq!(target, x_id),
        _ => panic!("Assignが期待される"),
    }
}
