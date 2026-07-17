use std::fs;
use std::path::PathBuf;
use tinycerilte::ast::{self, Program};
use tinycerilte::elaboration::{self, Elaborated};
use tinycerilte::netlist::{self, Netlist};
use tinycerilte::parser;
use tinycerilte::simulator;

/// コマンドライン引数
struct Cli {
    file: Option<String>,
    cycles: Option<u64>,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cli = parse_args(&args);
    let source = load_source(cli.file.as_deref());

    let prog = run_parse_phase(&source);
    let elab = run_elaboration_phase(&prog);
    let nl = run_netlist_phase(&elab);
    run_simulation_phase(&nl, cli.cycles);
}

/// `--cycles`/`-c` とファイルパスを引数から読み取る（不明なオプションは即終了）
fn parse_args(args: &[String]) -> Cli {
    let mut file = None;
    let mut cycles = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--cycles" | "-c" => {
                i += 1;
                cycles = Some(
                    args.get(i)
                        .and_then(|s| s.parse().ok())
                        .expect("--cycles には数値を指定"),
                );
            }
            flag if flag.starts_with('-') => {
                eprintln!("不明なオプション: {flag}");
                std::process::exit(1);
            }
            path => {
                file = Some(path.to_string());
            }
        }
        i += 1;
    }

    Cli { file, cycles }
}

/// ファイル指定があれば読み込み、なければ組み込みのサンプルコードを返す
fn load_source(file: Option<&str>) -> String {
    match file {
        Some(path) => fs::read_to_string(PathBuf::from(path)).unwrap_or_else(|e| {
            eprintln!("エラー: ファイル '{path}' を開けません: {e}");
            std::process::exit(1);
        }),
        None => r#"{
    var     a: bit;
    var     b: bit;

    a = b ^ 1;
    b <= a;
}"#
        .to_string(),
    }
}

/// Phase 1: パースし、結果をダンプする（失敗時は即終了）
fn run_parse_phase(source: &str) -> Program {
    println!("\n--- Phase 1: Parse ---");
    let prog = parser::Parser::parse_program(source).unwrap_or_else(|e| {
        eprintln!("  FAIL: {e}");
        std::process::exit(1);
    });

    print_parse_result(&prog);
    prog
}

/// Phase 1の結果（モジュール定義・ブロックの内訳）をダンプする
fn print_parse_result(prog: &Program) {
    println!("  OK: {} module(s), {} block(s)", prog.modules.len(), prog.blocks.len());
    for m in &prog.modules {
        println!("  Module {}: {} port(s), {} decl(s), {} stmt(s)", m.name, m.ports.len(), m.decls.len(), m.stmts.len());
    }
    for (i, block) in prog.blocks.iter().enumerate() {
        println!(
            "  Block {}: {} decl(s), {} instance(s), {} stmt(s)",
            i,
            block.decls.len(),
            block.instances.len(),
            block.stmts.len()
        );
        for decl in &block.decls {
            let width_str = decl.width.map(|w| format!("[{w}]")).unwrap_or_default();
            println!("    decl: {}: bit{}", decl.name, width_str);
        }
        for inst in &block.instances {
            println!("    inst: {} = {}(...)", inst.instance_name, inst.module_name);
        }
        for stmt in &block.stmts {
            match stmt {
                ast::Stmt::Combinational { target, expr } => {
                    println!("    comb: {target} = {expr:?}");
                }
                ast::Stmt::Sequential { target, expr } => {
                    println!("    seq:  {target} <= {expr:?}");
                }
            }
        }
    }
}

/// Phase 2: エラボレーションし、結果をダンプする（失敗時は即終了）
fn run_elaboration_phase(prog: &Program) -> Elaborated {
    println!("\n--- Phase 2: Elaboration ---");
    let elab = elaboration::elaborate(prog).unwrap_or_else(|e| {
        eprintln!("  FAIL: {e}");
        std::process::exit(1);
    });

    print_elaboration_result(&elab);
    elab
}

/// Phase 2の結果（モジュール数・信号一覧）をダンプする
fn print_elaboration_result(elab: &Elaborated) {
    println!("  OK: {} module(s), {} signal(s), {} stmt(s)", elab.modules.len(), elab.top.signals.len(), elab.top.stmts.len());
    for sig in &elab.top.signals {
        println!("  signal {}: {} ({} bit)", sig.id, sig.name, sig.width);
    }
}

/// Phase 3: ネットリストを構築し、テキスト表示する
fn run_netlist_phase(elab: &Elaborated) -> Netlist {
    println!("\n--- Phase 3: Netlist Build ---");
    let nl = netlist::build_netlist(elab);
    print_netlist_result(&nl);
    nl
}

/// Phase 3の結果（ネットリストのテキスト表現）をダンプする
fn print_netlist_result(nl: &Netlist) {
    println!();
    println!("{}", netlist::format_netlist(nl));
}

/// Phase 4: テストベンチの`initial`があればその手続きに従って実行し、
/// 無ければ`--cycles`が指定されている場合のみNサイクル実行する。
/// どちらの場合も結果は波形として表示する。
fn run_simulation_phase(nl: &Netlist, cycles: Option<u64>) {
    if !nl.initial.is_empty() {
        let snaps = run_initial_sequence(nl);
        print_simulation_result("Testbench (initial)", &snaps, nl);
        return;
    }

    let Some(n) = cycles else { return };

    let mut sim = simulator::Simulator::new(nl.signals.len());
    let snaps = if n > 0 { sim.run(&nl.nodes, n) } else { Vec::new() };
    print_simulation_result(&format!("Simulation ({n} cycles)"), &snaps, nl);
}

/// Phase 4の結果（波形）を表示する
fn print_simulation_result(phase_label: &str, snaps: &[simulator::CycleSnapshot], nl: &Netlist) {
    println!("--- Phase 4: {phase_label} ---\n");
    print!("{}", simulator::format_waveform(snaps, &nl.signals));
}

/// テストベンチの`initial`手続きを実行する: `Assign`はその場で値を設定し、
/// `Step`は1サイクル進めてスナップショットを記録する。
fn run_initial_sequence(nl: &Netlist) -> Vec<simulator::CycleSnapshot> {
    let mut sim = simulator::Simulator::new(nl.signals.len());
    let mut snaps = Vec::new();

    for step in &nl.initial {
        match step {
            netlist::InitialStep::Assign { target, expr_node } => {
                let width = nl.signals[*target].width;
                let value = simulator::eval_and_mask(*expr_node, &nl.nodes, sim.signal_values(), width);
                sim.set_signal(*target, value);
            }
            netlist::InitialStep::Step => {
                snaps.push(sim.step(&nl.nodes));
            }
        }
    }

    snaps
}
