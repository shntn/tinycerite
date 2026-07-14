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

    println!("  OK: {} block(s)", prog.blocks.len());
    for (i, block) in prog.blocks.iter().enumerate() {
        println!(
            "  Block {}: {} decl(s), {} stmt(s)",
            i,
            block.decls.len(),
            block.stmts.len()
        );
        for decl in &block.decls {
            let width_str = decl.width.map(|w| format!("[{w}]")).unwrap_or_default();
            println!("    decl: {}: bit{}", decl.name, width_str);
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
    prog
}

/// Phase 2: エラボレーションし、結果をダンプする（失敗時は即終了）
fn run_elaboration_phase(prog: &Program) -> Elaborated {
    println!("\n--- Phase 2: Elaboration ---");
    let elab = elaboration::elaborate(prog).unwrap_or_else(|e| {
        eprintln!("  FAIL: {e}");
        std::process::exit(1);
    });

    println!("  OK: {} signal(s), {} stmt(s)", elab.signals.len(), elab.stmts.len());
    for sig in &elab.signals {
        println!("  signal {}: {} ({} bit)", sig.id, sig.name, sig.width);
    }
    elab
}

/// Phase 3: ネットリストを構築し、テキスト表示する
fn run_netlist_phase(elab: &Elaborated) -> Netlist {
    println!("\n--- Phase 3: Netlist Build ---");
    let nl = netlist::build_netlist(elab);
    println!();
    println!("{}", netlist::format_netlist(&nl));
    nl
}

/// Phase 4: `--cycles` が指定されていればシミュレーションを実行し、波形を表示する
fn run_simulation_phase(nl: &Netlist, cycles: Option<u64>) {
    let Some(n) = cycles else { return };

    println!("--- Phase 4: Simulation ({n} cycles) ---\n");
    let mut sim = simulator::Simulator::new(nl.signals.len());
    if n > 0 {
        let snaps = sim.run(&nl.nodes, n);
        print!("{}", simulator::format_waveform(&snaps, &nl.signals));
    }
}
