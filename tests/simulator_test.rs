use tinycerilte::elaboration;
use tinycerilte::netlist;
use tinycerilte::parser::Parser;
use tinycerilte::simulator::{eval_and_mask, format_waveform, Simulator};

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

#[test]
fn logical_not_inverts_truthiness() {
    let (mut sim, nodes, _) = setup("{ var a: bit; var x: bit; a = 0; x = !a; }");
    let snap = sim.step(&nodes);
    assert_eq!(snap.values[1], 1, "!0 は真なので 1");
}

#[test]
fn logical_not_of_nonzero_is_zero() {
    let (mut sim, nodes, _) = setup("{ var a: bit<4>; var x: bit; a = 5; x = !a; }");
    let snap = sim.step(&nodes);
    assert_eq!(snap.values[1], 0, "!5 は偽なので 0");
}

#[test]
fn bitwise_not_masks_to_signal_width_at_assignment() {
    let (mut sim, nodes, _) = setup("{ var x: bit<4>; x = ~0; }");
    let snap = sim.step(&nodes);
    assert_eq!(snap.values[0], 15, "~0 は全ビット1、代入時に4ビットへ切り詰めて15");
}

#[test]
fn chained_unary_operators_evaluate_correctly() {
    let (mut sim, nodes, _) = setup("{ var x: bit; x = !!1; }");
    let snap = sim.step(&nodes);
    assert_eq!(snap.values[0], 1, "!!1 = !0 = 1");
}

#[test]
fn bitvec_literal_evaluates_to_declared_value() {
    let (mut sim, nodes, _) = setup("{ var x: bit<8>; x = 8'hFF; }");
    let snap = sim.step(&nodes);
    assert_eq!(snap.values[0], 255, "8'hFF = 255");
}

#[test]
fn bitvec_literal_overflowing_its_own_width_is_silently_masked() {
    let (mut sim, nodes, _) = setup("{ var x: bit<8>; x = 4'b11111; }");
    let snap = sim.step(&nodes);
    assert_eq!(snap.values[0], 15, "4'b11111は4ビットに切り詰められて0b1111=15");
}

#[test]
fn ternary_operator_selects_then_branch_when_cond_is_nonzero() {
    let (mut sim, nodes, _) = setup("{ var a: bit; var x: bit<4>; a = 1; x = a ? 5 : 9; }");
    let snap = sim.step(&nodes);
    assert_eq!(snap.values[1], 5, "condが真なのでthen(5)が選ばれる");
}

#[test]
fn ternary_operator_selects_else_branch_when_cond_is_zero() {
    let (mut sim, nodes, _) = setup("{ var a: bit; var x: bit<4>; a = 0; x = a ? 5 : 9; }");
    let snap = sim.step(&nodes);
    assert_eq!(snap.values[1], 9, "condが偽なのでelse(9)が選ばれる");
}

#[test]
fn module_instance_output_reflects_computed_value() {
    let (mut sim, nodes, signals) = setup(
        "module adder { port { a: input bit<8>; b: input bit<8>; sum: output bit<8>; } sum = a + b; } \
         { var x: bit<8>; var y: bit<8>; var z: bit<8>; x = 3; y = 4; var u1 = adder(a: x, b: y); z = u1.sum; }",
    );
    let z_id = signals.iter().position(|s| s.name == "z").unwrap();
    let snap = sim.step(&nodes);
    assert_eq!(snap.values[z_id], 7, "u1.sum = x + y = 3 + 4 = 7 がzに伝搬する");
}

#[test]
fn module_instance_with_sequential_output_has_one_cycle_latency() {
    let (mut sim, nodes, signals) = setup(
        "module adder { port { a: input bit<8>; b: input bit<8>; sum: output bit<8>; } sum <= a + b; } \
         { var x: bit<8>; var y: bit<8>; x = 3; y = 4; var u1 = adder(a: x, b: y); }",
    );
    let sum_id = signals.iter().position(|s| s.name == "u1.sum").unwrap();
    let snap0 = sim.step(&nodes);
    assert_eq!(snap0.values[sum_id], 0, "初回サイクルではsumは未反映(0)のまま");
    let snap1 = sim.step(&nodes);
    assert_eq!(snap1.values[sum_id], 7, "1サイクル遅れでsum=7が反映される");
}

/// `nl.initial`（testbenchのinitial手続き）をmain.rsのrun_initial_sequenceと同じ手順で実行し、
/// 最終的な信号値のリストを返す
fn run_initial(input: &str) -> (Simulator, Vec<netlist::Node>, Vec<netlist::NetlistSignal>) {
    let prog = Parser::parse_program(input).unwrap();
    let elab = elaboration::elaborate(&prog).unwrap();
    let nl = netlist::build_netlist(&elab);
    let mut sim = Simulator::new(nl.signals.len());

    for step in &nl.initial {
        match step {
            netlist::InitialStep::Assign { target, expr_node } => {
                let width = nl.signals[*target].width;
                let value = eval_and_mask(*expr_node, &nl.nodes, sim.signal_values(), width);
                sim.set_signal(*target, value);
            }
            netlist::InitialStep::Step => {
                sim.step(&nl.nodes);
            }
        }
    }

    (sim, nl.nodes, nl.signals)
}

#[test]
fn testbench_initial_assign_sets_value_immediately_without_stepping() {
    let (sim, _, signals) = run_initial("testbench tb { var x: bit<8>; initial { x = 3; } }");
    let x_id = signals.iter().position(|s| s.name == "x").unwrap();
    assert_eq!(sim.signal_values()[x_id], 3, "stepしなくてもinitialのassignは即座に反映される");
}

#[test]
fn testbench_initial_assign_is_masked_to_target_width() {
    let (sim, _, signals) = run_initial("testbench tb { var x: bit<4>; initial { x = 17; } }");
    let x_id = signals.iter().position(|s| s.name == "x").unwrap();
    assert_eq!(sim.signal_values()[x_id], 1, "17 & 0b1111 = 1（通常の代入と同じマスキング）");
}

#[test]
fn testbench_clock_divider_toggles_every_cycle() {
    // counter/clkはどちらもSequentialで、同じサイクル開始時点のsnapshotを参照するため、
    // clkはcounterの「サイクル開始時点」のLSBを反映する（counter自身のPhase2結果ではない）。
    // counter=0スタートなので: step1後clk=0(snapshot counter=0), step2後clk=1(snapshot counter=1)。
    let (sim, _, signals) = run_initial(
        "testbench tb { var counter: bit<8>; counter <= counter + 1; \
         var clk: bit; clk <= counter & 1; \
         initial { step; step; } }",
    );
    let clk_id = signals.iter().position(|s| s.name == "clk").unwrap();
    assert_eq!(sim.signal_values()[clk_id], 1, "2サイクル目でclkは0→1にトグルする");
}

#[test]
fn testbench_self_toggling_clock_flips_every_cycle() {
    let (sim, _, signals) =
        run_initial("testbench tb { var clk: bit; clk <= !clk; initial { step; } }");
    let clk_id = signals.iter().position(|s| s.name == "clk").unwrap();
    assert_eq!(sim.signal_values()[clk_id], 1, "clk <= !clk は自身のsnapshotを見るので毎ステップ確実にトグルする");
}

#[test]
fn testbench_drives_module_instance_and_reads_output_after_step() {
    let src = "module adder { port { a: input bit<8>; b: input bit<8>; sum: output bit<8>; } sum = a + b; } \
               testbench tb { var x: bit<8>; var y: bit<8>; var z: bit<8>; \
               var u1 = adder(a: x, b: y); \
               initial { x = 3; y = 4; step; z = u1.sum; } }";
    let (sim, _, signals) = run_initial(src);
    let z_id = signals.iter().position(|s| s.name == "z").unwrap();
    assert_eq!(sim.signal_values()[z_id], 7, "initial内でx,yを設定しstep後にu1.sumをzへ読み出せる");
}

#[test]
fn nested_ternary_operator_evaluates_right_associatively() {
    let (mut sim, nodes, _) =
        setup("{ var a: bit; var c: bit; var x: bit<4>; a = 0; c = 1; x = a ? 1 : c ? 2 : 3; }");
    let snap = sim.step(&nodes);
    assert_eq!(snap.values[2], 2, "a=0なのでelse側、c=1なのでthen(2)が選ばれる");
}
