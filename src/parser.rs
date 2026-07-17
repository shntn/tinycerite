use pest::iterators::Pair;
use pest::Parser as _;
use pest_derive::Parser as PestGrammar;

use crate::ast::*;

/// キーワードとして予約されている識別子
fn is_keyword(s: &str) -> bool {
    matches!(
        s,
        "var" | "bit" | "clock" | "module" | "port" | "input" | "output" | "testbench" | "initial" | "step"
    )
}

/// pest パーサー（grammar.pest から自動生成）
#[derive(PestGrammar)]
#[grammar = "grammar.pest"]
pub struct CeriteParser;

/// 構文解析エラー
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "パースエラー: {}", self.message)
    }
}

impl std::error::Error for ParseError {}

impl From<pest::error::Error<Rule>> for ParseError {
    fn from(e: pest::error::Error<Rule>) -> Self {
        ParseError {
            message: e.to_string(),
        }
    }
}

type Result<T> = std::result::Result<T, ParseError>;

/// パーサー（pest ラッパー）
pub struct Parser;

impl Parser {
    /// 入力文字列をパースし、AST の Program を返す
    pub fn parse_program(input: &str) -> Result<Program> {
        let pairs = CeriteParser::parse(Rule::program, input)?;
        let mut blocks = Vec::new();
        let mut modules = Vec::new();
        let mut testbenches = Vec::new();
        for pair in pairs {
            if pair.as_rule() == Rule::program {
                for item_pair in pair.into_inner() {
                    match item_pair.as_rule() {
                        Rule::block => blocks.push(parse_block(item_pair)?),
                        Rule::module_def => modules.push(parse_module_def(item_pair)?),
                        Rule::testbench_def => testbenches.push(parse_testbench_def(item_pair)?),
                        _ => {}
                    }
                }
            }
        }
        Ok(Program { blocks, modules, testbenches })
    }
}

fn parse_block(pair: Pair<Rule>) -> Result<Block> {
    let mut decls = Vec::new();
    let mut instances = Vec::new();
    let mut stmts = Vec::new();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::decl => decls.push(parse_decl(inner)?),
            Rule::inst_decl => instances.push(parse_inst_decl(inner)?),
            Rule::stmt => stmts.push(parse_stmt(inner)?),
            _ => {} // "{" "}" などは無視
        }
    }

    Ok(Block { decls, instances, stmts })
}

/// `module_def`（`"module" ~ ident ~ "{" ~ port_block ~ (decl | stmt)* ~ "}"`）をパースする
fn parse_module_def(pair: Pair<Rule>) -> Result<ModuleDef> {
    let mut name = String::new();
    let mut ports = Vec::new();
    let mut decls = Vec::new();
    let mut stmts = Vec::new();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::ident => {
                name = inner.as_str().to_string();
                if is_keyword(&name) {
                    return Err(ParseError {
                        message: format!("'{}' はキーワードでありモジュール名に使えません", name),
                    });
                }
            }
            Rule::port_block => ports = parse_port_block(inner)?,
            Rule::decl => decls.push(parse_decl(inner)?),
            Rule::stmt => stmts.push(parse_stmt(inner)?),
            _ => {} // "{" "}" などは無視
        }
    }

    Ok(ModuleDef { name, ports, decls, stmts })
}

/// `port_block`（`"port" ~ "{" ~ port_decl* ~ "}"`）をパースする
fn parse_port_block(pair: Pair<Rule>) -> Result<Vec<PortDecl>> {
    let mut ports = Vec::new();
    for inner in pair.into_inner() {
        if inner.as_rule() == Rule::port_decl {
            ports.push(parse_port_decl(inner)?);
        }
    }
    Ok(ports)
}

/// `port_decl`（`ident ~ ":" ~ direction ~ signal_type ~ ";"`）をパースする
fn parse_port_decl(pair: Pair<Rule>) -> Result<PortDecl> {
    let mut name = String::new();
    let mut direction = None;
    let mut sig_type = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::ident => {
                name = inner.as_str().to_string();
                if is_keyword(&name) {
                    return Err(ParseError {
                        message: format!("'{}' はキーワードでありポート名に使えません", name),
                    });
                }
            }
            Rule::direction => {
                direction = Some(match inner.as_str() {
                    "input" => Direction::Input,
                    "output" => Direction::Output,
                    _ => unreachable!("direction は input か output のみ"),
                });
            }
            Rule::signal_type => sig_type = Some(parse_signal_type(inner)?),
            _ => {} // ":" ";" は無視
        }
    }

    let direction = direction.ok_or_else(|| ParseError {
        message: "ポートの向き（input/output）が見つかりません".to_string(),
    })?;
    let sig_type = sig_type.ok_or_else(|| ParseError {
        message: "ポートの型（bit/clock）が見つかりません".to_string(),
    })?;

    Ok(PortDecl { name, direction, sig_type })
}

/// `signal_type`（`"clock" | ("bit" ~ ("<" ~ number ~ ">")?)`）をパースする
fn parse_signal_type(pair: Pair<Rule>) -> Result<SignalType> {
    if pair.as_str().starts_with("clock") {
        return Ok(SignalType::Clock);
    }

    let mut width = None;
    for inner in pair.into_inner() {
        if inner.as_rule() == Rule::number {
            width = Some(inner.as_str().parse::<u64>().map_err(|e| ParseError {
                message: format!("数値パース失敗: {} ({})", inner.as_str(), e),
            })?);
        }
    }
    Ok(SignalType::Bit(width))
}

/// `testbench_def`（`"testbench" ~ ident ~ "{" ~ (decl | inst_decl | stmt)* ~ initial_block? ~ "}"`）をパースする
fn parse_testbench_def(pair: Pair<Rule>) -> Result<TestbenchDef> {
    let mut name = String::new();
    let mut decls = Vec::new();
    let mut instances = Vec::new();
    let mut stmts = Vec::new();
    let mut initial = Vec::new();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::ident => {
                name = inner.as_str().to_string();
                if is_keyword(&name) {
                    return Err(ParseError {
                        message: format!("'{}' はキーワードでありテストベンチ名に使えません", name),
                    });
                }
            }
            Rule::decl => decls.push(parse_decl(inner)?),
            Rule::inst_decl => instances.push(parse_inst_decl(inner)?),
            Rule::stmt => stmts.push(parse_stmt(inner)?),
            Rule::initial_block => initial = parse_initial_block(inner)?,
            _ => {} // "{" "}" などは無視
        }
    }

    Ok(TestbenchDef { name, decls, instances, stmts, initial })
}

/// `initial_block`（`"initial" ~ "{" ~ (proc_assign | proc_step)* ~ "}"`）をパースする
fn parse_initial_block(pair: Pair<Rule>) -> Result<Vec<ProcStmt>> {
    let mut steps = Vec::new();
    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::proc_assign => steps.push(parse_proc_assign(inner)?),
            Rule::proc_step => steps.push(ProcStmt::Step),
            _ => {}
        }
    }
    Ok(steps)
}

/// `proc_assign`（`ident ~ "=" ~ ternary_expr ~ ";"`）をパースする
fn parse_proc_assign(pair: Pair<Rule>) -> Result<ProcStmt> {
    let mut target = String::new();
    let mut expr = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::ident => {
                target = inner.as_str().to_string();
                if is_keyword(&target) {
                    return Err(ParseError {
                        message: format!("'{}' はキーワードであり変数名に使えません", target),
                    });
                }
            }
            Rule::ternary_expr => expr = Some(parse_ternary_expr(inner)?),
            _ => {}
        }
    }

    let expr = expr.ok_or_else(|| ParseError {
        message: "式が見つかりません".to_string(),
    })?;

    Ok(ProcStmt::Assign { target, expr })
}

/// `inst_decl`（`"var" ~ ident ~ "=" ~ ident ~ "(" ~ (named_arg ~ ("," ~ named_arg)*)? ~ ")" ~ ";"`）をパースする
fn parse_inst_decl(pair: Pair<Rule>) -> Result<InstDecl> {
    let mut instance_name = None;
    let mut module_name = None;
    let mut args = Vec::new();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::ident if instance_name.is_none() => {
                let name = inner.as_str().to_string();
                if is_keyword(&name) {
                    return Err(ParseError {
                        message: format!("'{}' はキーワードでありインスタンス名に使えません", name),
                    });
                }
                instance_name = Some(name);
            }
            Rule::ident => module_name = Some(inner.as_str().to_string()),
            Rule::named_arg => args.push(parse_named_arg(inner)?),
            _ => {} // "var" "=" "(" ")" ";" は無視
        }
    }

    let instance_name = instance_name.ok_or_else(|| ParseError {
        message: "インスタンス名が見つかりません".to_string(),
    })?;
    let module_name = module_name.ok_or_else(|| ParseError {
        message: "モジュール名が見つかりません".to_string(),
    })?;

    Ok(InstDecl { instance_name, module_name, args })
}

/// `named_arg`（`ident ~ ":" ~ ternary_expr`）をパースする
fn parse_named_arg(pair: Pair<Rule>) -> Result<(String, Expr)> {
    let mut name = String::new();
    let mut expr = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::ident => name = inner.as_str().to_string(),
            Rule::ternary_expr => expr = Some(parse_ternary_expr(inner)?),
            _ => {}
        }
    }

    let expr = expr.ok_or_else(|| ParseError {
        message: "名前付き引数の式が見つかりません".to_string(),
    })?;

    Ok((name, expr))
}

fn parse_decl(pair: Pair<Rule>) -> Result<Decl> {
    let mut name = String::new();
    let mut sig_type = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::ident => {
                name = inner.as_str().to_string();
                if is_keyword(&name) {
                    return Err(ParseError {
                        message: format!("'{}' はキーワードであり変数名に使えません", name),
                    });
                }
            }
            Rule::signal_type => sig_type = Some(parse_signal_type(inner)?),
            _ => {} // "var" ":" ";" は無視
        }
    }

    let sig_type = sig_type.ok_or_else(|| ParseError {
        message: "変数の型（bit/clock）が見つかりません".to_string(),
    })?;

    Ok(Decl { name, sig_type })
}

fn parse_stmt(pair: Pair<Rule>) -> Result<Stmt> {
    let mut target = String::new();
    let mut is_seq = false;
    let mut expr = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::ident => {
                target = inner.as_str().to_string();
                if is_keyword(&target) {
                    return Err(ParseError {
                        message: format!("'{}' はキーワードであり変数名に使えません", target),
                    });
                }
            }
            Rule::assign => is_seq = false,
            Rule::nonblock => is_seq = true,
            Rule::ternary_expr => expr = Some(parse_ternary_expr(inner)?),
            _ => {} // ";" は無視
        }
    }

    let expr = expr.ok_or_else(|| ParseError {
        message: "式が見つかりません".to_string(),
    })?;

    if is_seq {
        Ok(Stmt::Sequential { target, expr })
    } else {
        Ok(Stmt::Combinational { target, expr })
    }
}

/// `ternary_expr`（`expression ~ ("?" ~ ternary_expr ~ ":" ~ ternary_expr)?`）を解決する
///
/// 条件部の後に `? ... : ...` が続かなければ、そのまま条件部の式を返す（三項演算子は
/// 常に使われるわけではないため）。続く場合は右結合で `Expr::Ternary` を組み立てる
/// （then/elseを再帰的に `parse_ternary_expr` で解決することで `a ? b : c ? d : e` が
/// `a ? b : (c ? d : e)` になる）。
fn parse_ternary_expr(pair: Pair<Rule>) -> Result<Expr> {
    let mut pairs = pair.into_inner();
    let cond_pair = pairs.next().ok_or_else(|| ParseError {
        message: "三項演算子の条件式が見つかりません".to_string(),
    })?;
    let cond = parse_expression(cond_pair)?;

    match (pairs.next(), pairs.next()) {
        (Some(then_pair), Some(else_pair)) => {
            let then_branch = parse_ternary_expr(then_pair)?;
            let else_branch = parse_ternary_expr(else_pair)?;
            Ok(Expr::Ternary {
                cond: Box::new(cond),
                then_branch: Box::new(then_branch),
                else_branch: Box::new(else_branch),
            })
        }
        _ => Ok(cond),
    }
}

/// 二項演算の優先順位チェーンの1段を左結合の `Expr::BinOp` 木に組み立てる
///
/// `pair` は `operand (op operand)*` の形をしたルールで、`parse_operand` で
/// オペランド1つを解決し、`op_from_str` で演算子ルールの文字列を `BinOp` に変換する。
fn parse_left_assoc(
    pair: Pair<Rule>,
    parse_operand: fn(Pair<Rule>) -> Result<Expr>,
    op_from_str: fn(&str) -> BinOp,
) -> Result<Expr> {
    let mut pairs = pair.into_inner();
    let first = pairs.next().ok_or_else(|| ParseError {
        message: "式に項がありません".to_string(),
    })?;
    let mut expr = parse_operand(first)?;

    while let Some(op_pair) = pairs.next() {
        let op = op_from_str(op_pair.as_str());
        let rhs_pair = pairs.next().ok_or_else(|| ParseError {
            message: "演算子の右辺が見つかりません".to_string(),
        })?;
        let rhs = parse_operand(rhs_pair)?;
        expr = Expr::BinOp {
            op,
            lhs: Box::new(expr),
            rhs: Box::new(rhs),
        };
    }

    Ok(expr)
}

fn parse_expression(pair: Pair<Rule>) -> Result<Expr> {
    parse_left_assoc(pair, parse_expression1, |_| BinOp::Or)
}

fn parse_expression1(pair: Pair<Rule>) -> Result<Expr> {
    parse_left_assoc(pair, parse_expression2, |_| BinOp::And)
}

fn parse_expression2(pair: Pair<Rule>) -> Result<Expr> {
    parse_left_assoc(pair, parse_expression3, |_| BinOp::BitOr)
}

fn parse_expression3(pair: Pair<Rule>) -> Result<Expr> {
    parse_left_assoc(pair, parse_expression4, |_| BinOp::Xor)
}

fn parse_expression4(pair: Pair<Rule>) -> Result<Expr> {
    parse_left_assoc(pair, parse_expression5, |_| BinOp::BitAnd)
}

fn parse_expression5(pair: Pair<Rule>) -> Result<Expr> {
    parse_left_assoc(pair, parse_expression6, |s| match s {
        "==" => BinOp::Eq,
        "!=" => BinOp::Neq,
        _ => unreachable!("eq_op は == か != のみ"),
    })
}

fn parse_expression6(pair: Pair<Rule>) -> Result<Expr> {
    parse_left_assoc(pair, parse_expression7, |s| match s {
        "<=" => BinOp::Le,
        "<" => BinOp::Lt,
        ">=" => BinOp::Ge,
        ">" => BinOp::Gt,
        _ => unreachable!("rel_op は <=, <, >=, > のみ"),
    })
}

fn parse_expression7(pair: Pair<Rule>) -> Result<Expr> {
    parse_left_assoc(pair, parse_expression8, |s| match s {
        "<<<" => BinOp::AShl,
        "<<" => BinOp::Shl,
        ">>>" => BinOp::AShr,
        ">>" => BinOp::Shr,
        _ => unreachable!("shift_op は <<<, <<, >>>, >> のみ"),
    })
}

fn parse_expression8(pair: Pair<Rule>) -> Result<Expr> {
    parse_left_assoc(pair, parse_expression9, |s| match s {
        "+" => BinOp::Add,
        "-" => BinOp::Sub,
        _ => unreachable!("add_op は + か - のみ"),
    })
}

fn parse_expression9(pair: Pair<Rule>) -> Result<Expr> {
    parse_left_assoc(pair, parse_expression_unary, |s| match s {
        "*" => BinOp::Mul,
        "/" => BinOp::Div,
        "%" => BinOp::Mod,
        _ => unreachable!("mul_op は *, /, % のみ"),
    })
}

/// 前置単項演算子の連鎖（`unary_op* ~ expression_factor`）を右結合の `Expr::UnaryOp` 木に組み立てる
fn parse_expression_unary(pair: Pair<Rule>) -> Result<Expr> {
    let mut ops = Vec::new();
    let mut operand = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::unary_op => ops.push(match inner.as_str() {
                "!" => UnOp::Not,
                "~" => UnOp::BitNot,
                _ => unreachable!("unary_op は ! か ~ のみ"),
            }),
            Rule::expression_factor => operand = Some(parse_expression_factor(inner)?),
            _ => {}
        }
    }

    let mut expr = operand.ok_or_else(|| ParseError {
        message: "式の項が見つかりません".to_string(),
    })?;
    for op in ops.into_iter().rev() {
        expr = Expr::UnaryOp {
            op,
            expr: Box::new(expr),
        };
    }
    Ok(expr)
}

fn parse_expression_factor(pair: Pair<Rule>) -> Result<Expr> {
    let inner = pair.into_inner().next().ok_or_else(|| ParseError {
        message: "式の項が見つかりません".to_string(),
    })?;

    match inner.as_rule() {
        Rule::ident => {
            let name = inner.as_str().to_string();
            if is_keyword(&name) {
                return Err(ParseError {
                    message: format!("'{}' はキーワードであり識別子として使えません", name),
                });
            }
            Ok(Expr::Ident(name))
        }
        Rule::number => {
            let n = inner
                .as_str()
                .parse::<u64>()
                .map_err(|e| ParseError {
                    message: format!("数値パース失敗: {} ({})", inner.as_str(), e),
                })?;
            Ok(Expr::Number(n))
        }
        Rule::bitvec_literal => parse_bitvec_literal(inner),
        Rule::field_access => parse_field_access(inner),
        Rule::ternary_expr => parse_ternary_expr(inner),
        _ => Err(ParseError {
            message: format!("式の項として予期しないルール: {:?}", inner.as_rule()),
        }),
    }
}

/// `field_access`（`ident ~ "." ~ ident`）を `Expr::FieldAccess` に変換する
fn parse_field_access(pair: Pair<Rule>) -> Result<Expr> {
    let mut idents = pair.into_inner();
    let instance = idents.next().ok_or_else(|| ParseError {
        message: "インスタンス名が見つかりません".to_string(),
    })?.as_str().to_string();
    let field = idents.next().ok_or_else(|| ParseError {
        message: "フィールド名が見つかりません".to_string(),
    })?.as_str().to_string();

    if is_keyword(&instance) || is_keyword(&field) {
        return Err(ParseError {
            message: format!("'{}.{}' にキーワードは使えません", instance, field),
        });
    }

    Ok(Expr::FieldAccess { instance, field })
}

/// `bitvec_literal`（`number ~ "'" ~ radix ~ literal_digits`）をパースする
///
/// 幅（`number`）と基数（`radix`: `b`=2進, `o`=8進, `d`=10進, `h`=16進）、桁の文字列
/// （`literal_digits`）を取り出し、基数に応じて数値へ変換する。基数に合わない桁
/// （例: `2'b19`のような`b`基数に対する`9`）はエラーになる。
fn parse_bitvec_literal(pair: Pair<Rule>) -> Result<Expr> {
    let mut width = None;
    let mut radix_char = None;
    let mut digits = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::number => {
                width = Some(inner.as_str().parse::<u64>().map_err(|e| ParseError {
                    message: format!("ビットベクタリテラルの幅パース失敗: {} ({})", inner.as_str(), e),
                })?);
            }
            Rule::radix => radix_char = Some(inner.as_str().to_string()),
            Rule::literal_digits => digits = Some(inner.as_str().to_string()),
            _ => {}
        }
    }

    let width = width.ok_or_else(|| ParseError {
        message: "ビットベクタリテラルの幅が見つかりません".to_string(),
    })?;
    let radix_char = radix_char.ok_or_else(|| ParseError {
        message: "ビットベクタリテラルの基数が見つかりません".to_string(),
    })?;
    let digits = digits.ok_or_else(|| ParseError {
        message: "ビットベクタリテラルの桁が見つかりません".to_string(),
    })?;

    let base = match radix_char.as_str() {
        "b" => 2,
        "o" => 8,
        "d" => 10,
        "h" => 16,
        _ => unreachable!("radix は b, o, d, h のみ"),
    };
    let value = u64::from_str_radix(&digits, base).map_err(|e| ParseError {
        message: format!(
            "ビットベクタリテラルの桁パース失敗: {}'{}{} ({})",
            width, radix_char, digits, e
        ),
    })?;

    Ok(Expr::BitVecLiteral { width, value })
}