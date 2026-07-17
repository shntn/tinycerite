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

/// エラボレーション結果
#[derive(Debug, Clone)]
pub struct Elaborated {
    pub signals: Vec<ResolvedSignal>,
    pub stmts: Vec<ResolvedStmt>,
}

/// シンボル名 → SignalId のマップ
pub type SymbolTable = HashMap<String, usize>;

/// エラボレーションを実行する
///
/// 宣言・文の解決（build）を済ませたあと、静的チェックを順に適用する。
/// チェックを追加する場合は `check_*` 関数を書き、ここに1行足すだけでよい。
pub fn elaborate(prog: &Program) -> Result<Elaborated> {
    let (signals, symtab) = build_signals(prog)?;
    let stmts = resolve_stmts(prog, &symtab)?;

    check_multiple_drivers(&stmts, &signals)?;
    check_combinational_loops(&stmts, &signals)?;

    Ok(Elaborated { signals, stmts })
}

/// 宣言を走査し、シンボルテーブルと解決済み信号リストを構築する（重複宣言はエラー）
fn build_signals(prog: &Program) -> Result<(Vec<ResolvedSignal>, SymbolTable)> {
    let mut signals = Vec::new();
    let mut symtab = SymbolTable::new();

    for block in &prog.blocks {
        for decl in &block.decls {
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
    }

    Ok((signals, symtab))
}

/// 代入文を走査し、変数名をシンボルIDに解決する（未宣言変数はエラー）
fn resolve_stmts(prog: &Program, symtab: &SymbolTable) -> Result<Vec<ResolvedStmt>> {
    let mut stmts = Vec::new();

    for block in &prog.blocks {
        for stmt in &block.stmts {
            let target = stmt.target();
            let target_id = *symtab.get(target).ok_or_else(|| ElabError {
                message: format!("変数 '{}' が宣言されていません", target),
            })?;
            let expr = resolve_expr(stmt.expr(), symtab)?;

            let resolved = match stmt {
                Stmt::Combinational { .. } => ResolvedStmt::Combinational { target_id, expr },
                Stmt::Sequential { .. } => ResolvedStmt::Sequential { target_id, expr },
            };
            stmts.push(resolved);
        }
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

fn resolve_expr(expr: &Expr, symtab: &SymbolTable) -> Result<ResolvedExpr> {
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
        Expr::BinOp { op, lhs, rhs } => {
            let lhs = resolve_expr(lhs, symtab)?;
            let rhs = resolve_expr(rhs, symtab)?;
            Ok(ResolvedExpr::BinOp {
                op: *op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            })
        }
        Expr::UnaryOp { op, expr } => {
            let expr = resolve_expr(expr, symtab)?;
            Ok(ResolvedExpr::UnaryOp {
                op: *op,
                expr: Box::new(expr),
            })
        }
        Expr::Ternary { cond, then_branch, else_branch } => {
            let cond = resolve_expr(cond, symtab)?;
            let then_branch = resolve_expr(then_branch, symtab)?;
            let else_branch = resolve_expr(else_branch, symtab)?;
            Ok(ResolvedExpr::Ternary {
                cond: Box::new(cond),
                then_branch: Box::new(then_branch),
                else_branch: Box::new(else_branch),
            })
        }
    }
}