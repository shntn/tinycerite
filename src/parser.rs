use pest::iterators::Pair;
use pest::Parser as _;
use pest_derive::Parser as PestGrammar;

use crate::ast::*;

/// キーワードとして予約されている識別子
fn is_keyword(s: &str) -> bool {
    matches!(s, "var" | "bit")
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
        for pair in pairs {
            if pair.as_rule() == Rule::program {
                for block_pair in pair.into_inner() {
                    if block_pair.as_rule() == Rule::block {
                        blocks.push(parse_block(block_pair)?);
                    }
                }
            }
        }
        Ok(Program { blocks })
    }
}

fn parse_block(pair: Pair<Rule>) -> Result<Block> {
    let mut decls = Vec::new();
    let mut stmts = Vec::new();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::decl => decls.push(parse_decl(inner)?),
            Rule::stmt => stmts.push(parse_stmt(inner)?),
            _ => {} // "{" "}" などは無視
        }
    }

    Ok(Block { decls, stmts })
}

fn parse_decl(pair: Pair<Rule>) -> Result<Decl> {
    let mut name = String::new();
    let mut width = None;

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
            Rule::number => {
                width = Some(
                    inner
                        .as_str()
                        .parse::<u64>()
                        .map_err(|e| ParseError {
                            message: format!("数値パース失敗: {} ({})", inner.as_str(), e),
                        })?,
                );
            }
            _ => {} // "var" ":" "bit" "<" ">" ";" は無視
        }
    }

    Ok(Decl { name, width })
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
            Rule::expr => expr = Some(parse_expr(inner)?),
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

fn parse_expr(pair: Pair<Rule>) -> Result<Expr> {
    let mut primaries: Vec<Expr> = Vec::new();

    // primary ("^" primary)* から primary のみ抽出し "^" は無視
    for inner in pair.into_inner() {
        if inner.as_rule() == Rule::primary {
            primaries.push(parse_primary(inner)?);
        }
    }

    // primary ("^" primary)* を左結合の BinOp 木に組み立てる
    // primaries = [a, b, c] → ((a ^ b) ^ c)
    let mut iter = primaries.into_iter();
    let first = iter.next().ok_or_else(|| ParseError {
        message: "式に項がありません".to_string(),
    })?;
    Ok(iter.fold(first, |lhs, rhs| Expr::BinOp {
        op: BinOp::Xor,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }))
}

fn parse_primary(pair: Pair<Rule>) -> Result<Expr> {
    let inner = pair
        .into_inner()
        .next()
        .ok_or_else(|| ParseError {
            message: "primary が空です".to_string(),
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
        _ => Err(ParseError {
            message: format!("primary の内部に予期しないルール: {:?}", inner.as_rule()),
        }),
    }
}