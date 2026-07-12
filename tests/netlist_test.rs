use tinycerilte::elaboration;
use tinycerilte::netlist::{self, DriveKind};
use tinycerilte::parser::Parser;

fn netlist_of(input: &str) -> netlist::Netlist {
    let prog = Parser::new(input).parse_program().unwrap();
    let elab = elaboration::elaborate(&prog).unwrap();
    netlist::build_netlist(&elab)
}

fn whole_pipeline(input: &str) -> String {
    let prog = Parser::new(input).parse_program().unwrap();
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