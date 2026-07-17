use crate::ast::*;
use std::collections::HashMap;
use std::collections::HashSet;

/// エラボレーションエラー
#[derive(Debug, Clone)]
pub struct ElabError {
    pub message: String,
}

impl std::fmt::Display for ElabError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "エラボレーションエラー: {}", self.message)
    }
}

impl std::error::Error for ElabError {}

type Result<T> = std::result::Result<T, ElabError>;

/// 解決済みの信号定義
#[derive(Debug, Clone)]
pub struct ResolvedSignal {
    pub name: String,
    pub width: u64,
    pub id: usize,
}

/// 解決済みの代入文
#[derive(Debug, Clone)]
pub enum ResolvedStmt {
    Combinational {
        target_id: usize,
        expr: ResolvedExpr,
    },
    Sequential {
        target_id: usize,
        expr: ResolvedExpr,
    },
}

/// 解決済みの式
#[derive(Debug, Clone)]
pub enum ResolvedExpr {
    Ident(usize), // signal_id
    Number(u64),
    BitVecLiteral {
        width: u64,
        value: u64,
    },
    /// モジュールインスタンスの出力ポート参照（`instance.field`）
    InstanceField {
        instance_name: String,
        port_name: String,
    },
    BinOp {
        op: BinOp,
        lhs: Box<ResolvedExpr>,
        rhs: Box<ResolvedExpr>,
    },
    UnaryOp {
        op: UnOp,
        expr: Box<ResolvedExpr>,
    },
    Ternary {
        cond: Box<ResolvedExpr>,
        then_branch: Box<ResolvedExpr>,
        else_branch: Box<ResolvedExpr>,
    },
}

/// 解決済みのポート定義（モジュール本体内でのローカル信号IDを持つ）
#[derive(Debug, Clone)]
pub struct ResolvedPort {
    pub name: String,
    pub direction: Direction,
    pub signal_id: usize,
}

/// 解決済みのモジュールインスタンス
#[derive(Debug, Clone)]
pub struct ResolvedInstance {
    pub instance_name: String,
    pub module_name: String,
    /// ポート名 → 接続式（呼び出し側スコープの信号を参照する解決済み式）
    pub connections: HashMap<String, ResolvedExpr>,
}

/// 信号・代入文・インスタンスの集合。トップレベルにもモジュール本体にも使う共通の形。
#[derive(Debug, Clone)]
pub struct ResolvedScope {
    pub signals: Vec<ResolvedSignal>,
    pub stmts: Vec<ResolvedStmt>,
    pub instances: Vec<ResolvedInstance>,
}

/// 解決済みのモジュール定義
#[derive(Debug, Clone)]
pub struct ResolvedModuleDef {
    pub name: String,
    pub ports: Vec<ResolvedPort>,
    pub body: ResolvedScope,
}

/// 解決済みの手続き文（`initial { }` 内）
#[derive(Debug, Clone)]
pub enum ResolvedProcStmt {
    Assign {
        target_id: usize,
        expr: ResolvedExpr,
    },
    Step,
}

/// エラボレーション結果
#[derive(Debug, Clone)]
pub struct Elaborated {
    pub top: ResolvedScope,
    pub modules: HashMap<String, ResolvedModuleDef>,
    pub initial: Vec<ResolvedProcStmt>,
}

/// シンボル名 → SignalId のマップ
pub type SymbolTable = HashMap<String, usize>;

/// インスタンス名 → モジュール名 のマップ（`FieldAccess` の解決に使う）
pub type InstanceTable = HashMap<String, String>;

/// エラボレーションを実行する
///
/// まずモジュール定義をそれぞれ独立に（インスタンス化の有無によらず）解決・検証し、
/// そのあとトップレベルのブロックを解決する。トップレベルは信号・代入文に加えて
/// モジュールインスタンスも解決するため、モジュール定義のテーブルを参照する。
/// 最後に、テストベンチの`initial`（手続き文）をトップレベルのシンボルテーブルで解決する。
pub fn elaborate(prog: &Program) -> Result<Elaborated> {
    if prog.testbenches.len() > 1 {
        return Err(ElabError {
            message: "テストベンチは1つだけ許可されています".to_string(),
        });
    }

    let modules = build_module_defs(prog)?;
    let (top, symtab, instance_table) = elaborate_top(prog, &modules)?;
    let initial = resolve_initial(prog, &symtab, &instance_table, &modules)?;
    Ok(Elaborated { top, modules, initial })
}

/// 全モジュール定義を1回ずつ解決・検証する（重複定義はエラー）
fn build_module_defs(prog: &Program) -> Result<HashMap<String, ResolvedModuleDef>> {
    let mut modules = HashMap::new();
    for m in &prog.modules {
        if modules.contains_key(&m.name) {
            return Err(ElabError {
                message: format!("モジュール '{}' が重複定義されています", m.name),
            });
        }
        let resolved = resolve_module_def(m)?;
        modules.insert(m.name.clone(), resolved);
    }
    Ok(modules)
}

/// モジュール定義を解決する: ポートを信号として登録し、本体の代入文を解決したうえで
/// 通常のスコープと同じ静的チェックを適用する
fn resolve_module_def(m: &ModuleDef) -> Result<ResolvedModuleDef> {
    let (mut signals, mut symtab, ports) = resolve_module_ports(m)?;
    resolve_module_decls(m, &mut signals, &mut symtab)?;
    let stmts = resolve_module_stmts(m, &symtab, &ports)?;

    check_multiple_drivers(&stmts, &signals)?;
    check_combinational_loops(&stmts, &signals)?;

    Ok(ResolvedModuleDef {
        name: m.name.clone(),
        ports,
        body: ResolvedScope { signals, stmts, instances: Vec::new() },
    })
}

/// モジュールのポート宣言を信号として登録する（重複ポート名はエラー）
fn resolve_module_ports(m: &ModuleDef) -> Result<(Vec<ResolvedSignal>, SymbolTable, Vec<ResolvedPort>)> {
    let mut signals = Vec::new();
    let mut symtab = SymbolTable::new();
    let mut ports = Vec::new();

    for p in &m.ports {
        if symtab.contains_key(&p.name) {
            return Err(ElabError {
                message: format!("モジュール '{}' のポート '{}' が重複しています", m.name, p.name),
            });
        }
        let id = signals.len();
        let width = p.width.unwrap_or(1);
        symtab.insert(p.name.clone(), id);
        signals.push(ResolvedSignal { name: p.name.clone(), width, id });
        ports.push(ResolvedPort { name: p.name.clone(), direction: p.direction, signal_id: id });
    }

    Ok((signals, symtab, ports))
}

/// モジュール本体の`var`宣言を、ポートと同じ信号空間に追加登録する（重複宣言はエラー）
fn resolve_module_decls(m: &ModuleDef, signals: &mut Vec<ResolvedSignal>, symtab: &mut SymbolTable) -> Result<()> {
    for decl in &m.decls {
        if symtab.contains_key(&decl.name) {
            return Err(ElabError {
                message: format!("モジュール '{}' 内で変数 '{}' が重複宣言されています", m.name, decl.name),
            });
        }
        let id = signals.len();
        let width = decl.width.unwrap_or(1);
        symtab.insert(decl.name.clone(), id);
        signals.push(ResolvedSignal { name: decl.name.clone(), width, id });
    }
    Ok(())
}

/// モジュール本体の代入文を解決する（入力ポートへの代入はエラー）。
/// モジュール本体は現状ネストしたインスタンス化を許可しない（文法上も不可）ため、
/// インスタンステーブル・モジュールテーブルは空で`resolve_expr`に渡す。
fn resolve_module_stmts(m: &ModuleDef, symtab: &SymbolTable, ports: &[ResolvedPort]) -> Result<Vec<ResolvedStmt>> {
    let instances = InstanceTable::new();
    let empty_modules = HashMap::new();

    let mut stmts = Vec::new();
    for stmt in &m.stmts {
        let target = stmt.target();
        let target_id = *symtab.get(target).ok_or_else(|| ElabError {
            message: format!("モジュール '{}': 変数 '{}' が宣言されていません", m.name, target),
        })?;
        if let Some(port) = ports.iter().find(|p| p.signal_id == target_id)
            && port.direction == Direction::Input
        {
            return Err(ElabError {
                message: format!("モジュール '{}': 入力ポート '{}' に代入できません", m.name, target),
            });
        }
        let expr = resolve_expr(stmt.expr(), symtab, &instances, &empty_modules)?;

        let resolved = match stmt {
            Stmt::Combinational { .. } => ResolvedStmt::Combinational { target_id, expr },
            Stmt::Sequential { .. } => ResolvedStmt::Sequential { target_id, expr },
        };
        stmts.push(resolved);
    }

    Ok(stmts)
}

/// トップレベルのブロック群（とテストベンチの並行部分）を解決する:
/// 信号・インスタンス・代入文の順に解決し、静的チェックを適用する。
/// 後段の`resolve_initial`が同じシンボルテーブル・インスタンステーブルを使えるよう、
/// スコープと一緒に返す。
fn elaborate_top(
    prog: &Program,
    modules: &HashMap<String, ResolvedModuleDef>,
) -> Result<(ResolvedScope, SymbolTable, InstanceTable)> {
    let (signals, symtab) = build_signals(prog)?;
    let (instances, instance_table) = build_instances(prog, &symtab, modules)?;
    let stmts = resolve_stmts(prog, &symtab, &instance_table, modules)?;

    check_multiple_drivers(&stmts, &signals)?;
    check_combinational_loops(&stmts, &signals)?;

    Ok((ResolvedScope { signals, stmts, instances }, symtab, instance_table))
}

/// テストベンチの`initial`（手続き文）を解決する。
/// 対象信号はトップレベルのシンボルテーブルで解決する（`initial`はモジュール本体を持てない）。
fn resolve_initial(
    prog: &Program,
    symtab: &SymbolTable,
    instances: &InstanceTable,
    modules: &HashMap<String, ResolvedModuleDef>,
) -> Result<Vec<ResolvedProcStmt>> {
    let mut result = Vec::new();

    for tb in &prog.testbenches {
        for step in &tb.initial {
            let resolved = match step {
                ProcStmt::Assign { target, expr } => {
                    let target_id = *symtab.get(target).ok_or_else(|| ElabError {
                        message: format!("変数 '{}' が宣言されていません", target),
                    })?;
                    let expr = resolve_expr(expr, symtab, instances, modules)?;
                    ResolvedProcStmt::Assign { target_id, expr }
                }
                ProcStmt::Step => ResolvedProcStmt::Step,
            };
            result.push(resolved);
        }
    }

    Ok(result)
}

/// 宣言を走査し、シンボルテーブルと解決済み信号リストを構築する（重複宣言はエラー）
///
/// トップレベルのブロックとテストベンチの並行部分は、どちらも同じフラットな
/// 信号空間に合流する（テストベンチの`decls`もここで一緒に登録される）。
fn build_signals(prog: &Program) -> Result<(Vec<ResolvedSignal>, SymbolTable)> {
    let mut signals = Vec::new();
    let mut symtab = SymbolTable::new();

    let all_decls = prog
        .blocks
        .iter()
        .flat_map(|b| &b.decls)
        .chain(prog.testbenches.iter().flat_map(|t| &t.decls));

    for decl in all_decls {
        if symtab.contains_key(&decl.name) {
            return Err(ElabError {
                message: format!("変数 '{}' が重複宣言されています", decl.name),
            });
        }
        let id = signals.len();
        let width = decl.width.unwrap_or(1);
        symtab.insert(decl.name.clone(), id);
        signals.push(ResolvedSignal {
            name: decl.name.clone(),
            width,
            id,
        });
    }

    Ok((signals, symtab))
}

/// モジュールインスタンス化を走査し、引数をポート定義と突き合わせて解決する
fn build_instances(
    prog: &Program,
    symtab: &SymbolTable,
    modules: &HashMap<String, ResolvedModuleDef>,
) -> Result<(Vec<ResolvedInstance>, InstanceTable)> {
    let mut instances = Vec::new();
    let mut instance_table = InstanceTable::new();
    // インスタンス化の接続式は同じスコープ内の他インスタンスの出力も参照できるため、
    // ここまでに解決済みのインスタンスも見えるようにテーブルを育てながら解決する
    let mut resolved_so_far: HashMap<String, String> = HashMap::new();

    let all_instances = prog
        .blocks
        .iter()
        .flat_map(|b| &b.instances)
        .chain(prog.testbenches.iter().flat_map(|t| &t.instances));

    for inst in all_instances {
        if symtab.contains_key(&inst.instance_name) || instance_table.contains_key(&inst.instance_name) {
            return Err(ElabError {
                message: format!("'{}' という名前が信号またはインスタンスと重複しています", inst.instance_name),
            });
        }
        let module_def = modules.get(&inst.module_name).ok_or_else(|| ElabError {
            message: format!("モジュール '{}' が定義されていません", inst.module_name),
        })?;

        let input_ports: Vec<&ResolvedPort> =
            module_def.ports.iter().filter(|p| p.direction == Direction::Input).collect();

        let mut connections = HashMap::new();
        for (arg_name, arg_expr) in &inst.args {
            let is_input_port = input_ports.iter().any(|p| &p.name == arg_name);
            if !is_input_port {
                let is_output_port = module_def.ports.iter().any(|p| &p.name == arg_name && p.direction == Direction::Output);
                let message = if is_output_port {
                    format!(
                        "'{}' はモジュール '{}' の出力ポートです。インスタンス化時には入力ポートのみ指定できます",
                        arg_name, inst.module_name
                    )
                } else {
                    format!("モジュール '{}' に入力ポート '{}' はありません", inst.module_name, arg_name)
                };
                return Err(ElabError { message });
            }
            if connections.contains_key(arg_name) {
                return Err(ElabError {
                    message: format!("引数 '{}' が重複しています", arg_name),
                });
            }
            let resolved_expr = resolve_expr(arg_expr, symtab, &resolved_so_far, modules)?;
            connections.insert(arg_name.clone(), resolved_expr);
        }

        for p in &input_ports {
            if !connections.contains_key(&p.name) {
                return Err(ElabError {
                    message: format!(
                        "モジュール '{}' の入力ポート '{}' が接続されていません",
                        inst.module_name, p.name
                    ),
                });
            }
        }

        instance_table.insert(inst.instance_name.clone(), inst.module_name.clone());
        resolved_so_far.insert(inst.instance_name.clone(), inst.module_name.clone());
        instances.push(ResolvedInstance {
            instance_name: inst.instance_name.clone(),
            module_name: inst.module_name.clone(),
            connections,
        });
    }

    Ok((instances, instance_table))
}

/// 代入文を走査し、変数名をシンボルIDに解決する（未宣言変数はエラー）
fn resolve_stmts(
    prog: &Program,
    symtab: &SymbolTable,
    instances: &InstanceTable,
    modules: &HashMap<String, ResolvedModuleDef>,
) -> Result<Vec<ResolvedStmt>> {
    let mut stmts = Vec::new();

    let all_stmts = prog
        .blocks
        .iter()
        .flat_map(|b| &b.stmts)
        .chain(prog.testbenches.iter().flat_map(|t| &t.stmts));

    for stmt in all_stmts {
        let target = stmt.target();
        let target_id = *symtab.get(target).ok_or_else(|| ElabError {
            message: format!("変数 '{}' が宣言されていません", target),
        })?;
        let expr = resolve_expr(stmt.expr(), symtab, instances, modules)?;

        let resolved = match stmt {
            Stmt::Combinational { .. } => ResolvedStmt::Combinational { target_id, expr },
            Stmt::Sequential { .. } => ResolvedStmt::Sequential { target_id, expr },
        };
        stmts.push(resolved);
    }

    Ok(stmts)
}

/// 同一信号への複数ドライバ（多重代入）を検出する
fn check_multiple_drivers(stmts: &[ResolvedStmt], signals: &[ResolvedSignal]) -> Result<()> {
    let mut driven = HashSet::new();
    for stmt in stmts {
        let target_id = match stmt {
            ResolvedStmt::Combinational { target_id, .. }
            | ResolvedStmt::Sequential { target_id, .. } => *target_id,
        };
        if !driven.insert(target_id) {
            return Err(ElabError {
                message: format!(
                    "変数 '{}' に複数のドライバがあります（多重代入）",
                    signals[target_id].name
                ),
            });
        }
    }
    Ok(())
}

const WHITE: u8 = 0;
const GRAY: u8 = 1;
const BLACK: u8 = 2;

/// 組合せ依存グラフの循環を検出する
///
/// 備考: モジュールインスタンスの出力（`InstanceField`）を読む式は、そのインスタンスの
/// 入力ポートに何が繋がっているかに関わらず依存なしの葉として扱う。そのため、インスタンスの
/// 出力と入力がスコープをまたいで組合せループを作るケース（例: `u1.a = z; z = u1.sum;`）は
/// ここでは検出できず、ネットリスト構築後にシミュレータのΔ-サイクル上限で検出されることになる
/// （エラーにはなるが、エラボレーション時点より遅いタイミングでの検出になる）。
fn check_combinational_loops(stmts: &[ResolvedStmt], signals: &[ResolvedSignal]) -> Result<()> {
    let deps = build_combinational_deps(stmts, signals.len());

    let mut color = vec![WHITE; signals.len()];
    let mut path = Vec::new();
    for i in 0..signals.len() {
        if color[i] == WHITE {
            dfs_visit(i, &deps, &mut color, &mut path, signals)?;
        }
    }
    Ok(())
}

/// 組合せ代入の依存グラフを構築する: deps[信号ID] = その信号を右辺で読む組合せDriveのターゲットID一覧
fn build_combinational_deps(stmts: &[ResolvedStmt], signal_count: usize) -> Vec<Vec<usize>> {
    let mut deps: Vec<Vec<usize>> = vec![vec![]; signal_count];
    for stmt in stmts {
        if let ResolvedStmt::Combinational { target_id, expr } = stmt {
            for read in collect_read_signals(expr) {
                deps[read].push(*target_id);
            }
        }
    }
    deps
}

/// 依存グラフをDFSで訪問し、循環を検出する（経路を保持し、見つかった循環をエラーに含める）
fn dfs_visit(
    node: usize,
    deps: &[Vec<usize>],
    color: &mut [u8],
    path: &mut Vec<usize>,
    signals: &[ResolvedSignal],
) -> Result<()> {
    color[node] = GRAY;
    path.push(node);
    for &next in &deps[node] {
        if color[next] == GRAY {
            return Err(cycle_error(path, next, signals));
        }
        if color[next] == WHITE {
            dfs_visit(next, deps, color, path, signals)?;
        }
    }
    path.pop();
    color[node] = BLACK;
    Ok(())
}

/// 探索中に見つかった循環から、経路を含むエラーメッセージを組み立てる
fn cycle_error(path: &[usize], next: usize, signals: &[ResolvedSignal]) -> ElabError {
    let cycle_start = path.iter().position(|&x| x == next).unwrap();
    let cycle: Vec<&str> = path[cycle_start..]
        .iter()
        .chain(std::iter::once(&next))
        .map(|&i| signals[i].name.as_str())
        .collect();
    ElabError {
        message: format!(
            "組合せループを検出: {} → {} → ... → {} の循環があります",
            cycle[0], cycle[1], cycle[cycle.len() - 2]
        ),
    }
}

/// 解決済み式から参照される信号IDを収集
fn collect_read_signals(expr: &ResolvedExpr) -> Vec<usize> {
    match expr {
        ResolvedExpr::Ident(id) => vec![*id],
        ResolvedExpr::Number(_) => vec![],
        ResolvedExpr::BitVecLiteral { .. } => vec![],
        ResolvedExpr::InstanceField { .. } => vec![],
        ResolvedExpr::BinOp { lhs, rhs, .. } => {
            let mut v = collect_read_signals(lhs);
            v.extend(collect_read_signals(rhs));
            v
        }
        ResolvedExpr::UnaryOp { expr, .. } => collect_read_signals(expr),
        ResolvedExpr::Ternary { cond, then_branch, else_branch } => {
            let mut v = collect_read_signals(cond);
            v.extend(collect_read_signals(then_branch));
            v.extend(collect_read_signals(else_branch));
            v
        }
    }
}

fn resolve_expr(
    expr: &Expr,
    symtab: &SymbolTable,
    instances: &InstanceTable,
    modules: &HashMap<String, ResolvedModuleDef>,
) -> Result<ResolvedExpr> {
    match expr {
        Expr::Ident(name) => {
            let id = symtab.get(name).ok_or_else(|| ElabError {
                message: format!("変数 '{}' が宣言されていません", name),
            })?;
            Ok(ResolvedExpr::Ident(*id))
        }
        Expr::Number(n) => Ok(ResolvedExpr::Number(*n)),
        Expr::BitVecLiteral { width, value } => Ok(ResolvedExpr::BitVecLiteral {
            width: *width,
            value: *value,
        }),
        Expr::FieldAccess { instance, field } => {
            let module_name = instances.get(instance).ok_or_else(|| ElabError {
                message: format!("インスタンス '{}' が見つかりません", instance),
            })?;
            let module_def = modules.get(module_name).ok_or_else(|| ElabError {
                message: format!("モジュール '{}' が定義されていません", module_name),
            })?;
            let port = module_def.ports.iter().find(|p| &p.name == field).ok_or_else(|| ElabError {
                message: format!("'{}' はモジュール '{}' のポートではありません", field, module_name),
            })?;
            if port.direction != Direction::Output {
                return Err(ElabError {
                    message: format!("'{}.{}' は入力ポートのため読み出せません", instance, field),
                });
            }
            Ok(ResolvedExpr::InstanceField {
                instance_name: instance.clone(),
                port_name: field.clone(),
            })
        }
        Expr::BinOp { op, lhs, rhs } => {
            let lhs = resolve_expr(lhs, symtab, instances, modules)?;
            let rhs = resolve_expr(rhs, symtab, instances, modules)?;
            Ok(ResolvedExpr::BinOp {
                op: *op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            })
        }
        Expr::UnaryOp { op, expr } => {
            let expr = resolve_expr(expr, symtab, instances, modules)?;
            Ok(ResolvedExpr::UnaryOp {
                op: *op,
                expr: Box::new(expr),
            })
        }
        Expr::Ternary { cond, then_branch, else_branch } => {
            let cond = resolve_expr(cond, symtab, instances, modules)?;
            let then_branch = resolve_expr(then_branch, symtab, instances, modules)?;
            let else_branch = resolve_expr(else_branch, symtab, instances, modules)?;
            Ok(ResolvedExpr::Ternary {
                cond: Box::new(cond),
                then_branch: Box::new(then_branch),
                else_branch: Box::new(else_branch),
            })
        }
    }
}
