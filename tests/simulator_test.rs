use tinycerilte::elaboration;
use tinycerilte::netlist;
use tinycerilte::parser::Parser;
use tinycerilte::simulator::{format_waveform, Simulator};

fn setup(input: &str) -> (Simulator, Vec<netlist::Node>, Vec<netlist::NetlistSignal>) {
    let prog = Parser::parse_program(input).unwrap();
    let elab = elaboration::elaborate(&prog).unwrap();
    let nl = netlist::build_netlist(&elab);
    let sim = Simulator::new(nl.signals.len());
    (sim, nl.nodes, nl.signals)
}

#[test]
fn constant_assign_is_stable() {
    let (mut sim, nodes, _) = setup("{ var x: bit; x = 1; }");
    let snap = sim.step(&nodes);
    assert_eq!(snap.values[0], 1);
}

#[test]
fn xor_combinational() {
    let (mut sim, nodes, _) = setup("{ var a: bit; var b: bit; a = b ^ 1; }");
    let snap = sim.step(&nodes);
    assert_eq!(snap.values[0], 1, "a = 0 ^ 1 = 1");
}

#[test]
fn non_blocking_samples_at_cycle_start() {
    let (mut sim, nodes, _) = setup("{ var a: bit; var b: bit; a = 1; b <= a; }");
    // Cycle 0: a=1 (comb), b はサイクル開始時の a=0 を取る
    let snap0 = sim.step(&nodes);
    assert_eq!(snap0.values[0], 1);
    assert_eq!(snap0.values[1], 0, "b <= a は開始時点の a=0 を参照");

    // Cycle 1: b が a=1 を取り込む
    let snap1 = sim.step(&nodes);
    assert_eq!(snap1.values[1], 1, "1サイクル遅れで b=1");
}

#[test]
fn toggle_flip_flop_pattern() {
    let (mut sim, nodes, _) =
        setup("{ var a: bit; var b: bit; a = b ^ 1; b <= a; }");
    let snaps = sim.run(&nodes, 6);
    assert_eq!(snaps.len(), 6);

    let a_vals: Vec<u64> = snaps.iter().map(|s| s.values[0]).collect();
    let b_vals: Vec<u64> = snaps.iter().map(|s| s.values[1]).collect();

    // a: b XOR 1（組み合わせ）のトグル
    assert_eq!(a_vals, vec![1, 1, 0, 0, 1, 1]);
    // b: a を1サイクル遅れで追従
    assert_eq!(b_vals, vec![0, 1, 1, 0, 0, 1]);
}

#[test]
fn set_initial_value() {
    let (mut sim, nodes, _) = setup("{ var x: bit; x = 0; }");
    sim.set_signal(0, 1);
    let snap = sim.step(&nodes);
    // comb: x = 0^1... wait, x = 0 means Const(0). x starts at 1, then gets 0
    assert_eq!(snap.values[0], 0, "x = 0 なので 0 で上書き");
}

#[test]
fn waveform_format_includes_header_and_data() {
    let (mut sim, nodes, signals) =
        setup("{ var a: bit; var b: bit; a = b ^ 1; b <= a; }");
    let snaps = sim.run(&nodes, 4);
    let text = format_waveform(&snaps, &signals);
    assert!(text.contains("cycle"), "ヘッダーに cycle");
    assert!(text.contains("a"), "ヘッダーに a");
    assert!(text.contains("b"), "ヘッダーに b");
    assert!(text.contains("  0"), "0サイクル目の行");
}

#[test]
fn empty_waveform_on_no_snapshots() {
    let text = format_waveform(&[], &[]);
    assert_eq!(text, "", "空の場合は空文字列");
}

#[test]
#[should_panic(expected = "組合せループ")]
fn combinational_loop_detected() {
    let (mut sim, nodes, _) = setup("{ var a: bit; a = a ^ 1; }");
    sim.step(&nodes); // 収束しないのでパニック
}

#[test]
fn chained_xor_evaluates_left_to_right_associatively() {
    let (mut sim, nodes, _) = setup(
        "{ var a: bit; var b: bit; var c: bit; var d: bit; a = 1; b = 0; c = 1; d = a ^ b ^ c; }",
    );
    let snap = sim.step(&nodes);
    assert_eq!(snap.values[3], 0, "d = 1 ^ 0 ^ 1 = 0");
}

#[test]
fn arithmetic_and_precedence_evaluate_correctly() {
    let (mut sim, nodes, _) = setup("{ var x: bit<8>; x = 1 + 2 * 3; }");
    let snap = sim.step(&nodes);
    assert_eq!(snap.values[0], 7, "1 + 2 * 3 = 7（* が + より先に評価される）");
}

#[test]
fn comparison_operator_yields_boolean_value() {
    let (mut sim, nodes, _) = setup("{ var x: bit; x = 3 < 5; }");
    let snap = sim.step(&nodes);
    assert_eq!(snap.values[0], 1, "3 < 5 は真なので 1");
}

#[test]
fn logical_and_short_circuits_to_zero_when_lhs_is_zero() {
    let (mut sim, nodes, _) = setup("{ var x: bit; x = 0 && 1; }");
    let snap = sim.step(&nodes);
    assert_eq!(snap.values[0], 0, "0 && 1 は偽なので 0");
}

#[test]
fn shift_operators_evaluate_correctly() {
    let (mut sim, nodes, _) = setup("{ var x: bit<8>; x = 1 << 4; }");
    let snap = sim.step(&nodes);
    assert_eq!(snap.values[0], 16, "1 << 4 = 16");
}

#[test]
fn division_by_zero_yields_zero_instead_of_panicking() {
    let (mut sim, nodes, _) = setup("{ var x: bit; x = 5 / 0; }");
    let snap = sim.step(&nodes);
    assert_eq!(snap.values[0], 0, "0除算は未定義値の代わりに0を返す");
}

#[test]
fn combinational_assign_masks_value_to_signal_width() {
    let (mut sim, nodes, _) = setup("{ var x: bit<4>; x = 17; }");
    let snap = sim.step(&nodes);
    assert_eq!(snap.values[0], 1, "17 & 0b1111 = 1（4ビットに切り詰め）");
}

#[test]
fn sequential_assign_masks_value_to_signal_width() {
    let (mut sim, nodes, _) = setup("{ var b: bit<4>; b <= 17; }");
    let snap = sim.step(&nodes);
    assert_eq!(snap.values[0], 1, "17 & 0b1111 = 1（順序代入でも4ビットに切り詰め）");
}
