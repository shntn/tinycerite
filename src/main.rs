use std::fs;
use std::path::PathBuf;
use tinycerilte::ast;
use tinycerilte::elaboration;
use tinycerilte::lexer;
use tinycerilte::netlist;
use tinycerilte::parser;
use tinycerilte::simulator;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let mut file_arg: Option<&str> = None;
    let mut cycles: Option<u64> = None;

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
                file_arg = Some(path);
            }
        }
        i += 1;
    }

    let source = match file_arg {
        Some(path) => match fs::read_to_string(PathBuf::from(path)) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("エラー: ファイル '{path}' を開けません: {e}");
                std::process::exit(1);
            }
        },
        None => {
            r#"{
    var     a: bit;
    var     b: bit;

    a = b ^ 1;
    b <= a;
}"#
            .to_string()
        }
    };

    // Phase 1: Lex
    println!("--- Phase 1: Lex ---");
    {
        let mut lex = lexer::Lexer::new(&source);
        loop {
            let tok = lex.next_token();
            if matches!(tok, lexer::Token::Eof) {
                break;
            }
            println!("  {:?}", tok);
        }
    }

    // Phase 2: Parse
    println!("\n--- Phase 2: Parse ---");
    let mut parse = parser::Parser::new(&source);
    let prog = match parse.parse_program() {
        Ok(p) => {
            println!("  OK: {} block(s)", p.blocks.len());
            for (i, block) in p.blocks.iter().enumerate() {
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
            p
        }
        Err(e) => {
            eprintln!("  FAIL: {e}");
            std::process::exit(1);
        }
    };

    // Phase 3: Elaboration
    println!("\n--- Phase 3: Elaboration ---");
    let elab = match elaboration::elaborate(&prog) {
        Ok(e) => {
            println!("  OK: {} signal(s), {} stmt(s)", e.signals.len(), e.stmts.len());
            for sig in &e.signals {
                println!("  signal {}: {} ({} bit)", sig.id, sig.name, sig.width);
            }
            e
        }
        Err(e) => {
            eprintln!("  FAIL: {e}");
            std::process::exit(1);
        }
    };

    // Phase 4: Netlist
    println!("\n--- Phase 4: Netlist Build ---");
    let nl = netlist::build_netlist(&elab);
    println!();
    println!("{}", netlist::format_netlist(&nl));

    // Phase 5: Simulation (if requested)
    if let Some(n) = cycles {
        println!("--- Phase 5: Simulation ({n} cycles) ---\n");
        let mut sim = simulator::Simulator::new(nl.signals.len());
        if n > 0 {
            let snaps = sim.run(&nl.nodes, n);
            print!("{}", simulator::format_waveform(&snaps, &nl.signals));
        }
    }
}