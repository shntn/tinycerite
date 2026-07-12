use std::fmt;

/// プログラム全体（現状は1ブロックのみ）
#[derive(Debug, Clone)]
pub struct Program {
    pub blocks: Vec<Block>,
}

/// ブロック
#[derive(Debug, Clone)]
pub struct Block {
    pub decls: Vec<Decl>,
    pub stmts: Vec<Stmt>,
}

/// 変数宣言
#[derive(Debug, Clone)]
pub struct Decl {
    pub name: String,
    pub width: Option<u64>, // None = bit (1-bit), Some(n) = bit[n]
}

/// 代入文
#[derive(Debug, Clone)]
pub enum Stmt {
    /// 組み合わせ代入 `target = expr`
    Combinational {
        target: String,
        expr: Expr,
    },
    /// 順序代入 `target <= expr`
    Sequential {
        target: String,
        expr: Expr,
    },
}

impl Stmt {
    pub fn target(&self) -> &str {
        match self {
            Stmt::Combinational { target, .. } | Stmt::Sequential { target, .. } => target,
        }
    }

    pub fn expr(&self) -> &Expr {
        match self {
            Stmt::Combinational { expr, .. } | Stmt::Sequential { expr, .. } => expr,
        }
    }
}

/// 式
#[derive(Debug, Clone)]
pub enum Expr {
    /// 変数参照
    Ident(String),
    /// 数値リテラル（10進数）
    Number(u64),
    /// 二項演算
    BinOp {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
}

/// 二項演算子
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp {
    Xor,
}

impl fmt::Display for BinOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BinOp::Xor => write!(f, "^"),
        }
    }
}