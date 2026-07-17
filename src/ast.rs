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
    /// 数値リテラル（10進数、幅は代入先や周囲の式から推測される）
    Number(u64),
    /// ビットベクタリテラル（例: `4'b1010`、`8'hFF`）。幅を明示する
    BitVecLiteral {
        width: u64,
        value: u64,
    },
    /// 二項演算
    BinOp {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    /// 単項演算
    UnaryOp {
        op: UnOp,
        expr: Box<Expr>,
    },
    /// 三項演算（条件式） `cond ? then_branch : else_branch`
    Ternary {
        cond: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Box<Expr>,
    },
}

/// 単項演算子
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnOp {
    /// 論理否定
    Not,
    /// ビット反転
    BitNot,
}

impl fmt::Display for UnOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            UnOp::Not => "!",
            UnOp::BitNot => "~",
        };
        write!(f, "{s}")
    }
}

/// 二項演算子
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp {
    Or,
    And,
    BitOr,
    Xor,
    BitAnd,
    Eq,
    Neq,
    Lt,
    Le,
    Gt,
    Ge,
    Shl,
    Shr,
    AShl,
    AShr,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

impl fmt::Display for BinOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            BinOp::Or => "||",
            BinOp::And => "&&",
            BinOp::BitOr => "|",
            BinOp::Xor => "^",
            BinOp::BitAnd => "&",
            BinOp::Eq => "==",
            BinOp::Neq => "!=",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
            BinOp::Shl => "<<",
            BinOp::Shr => ">>",
            BinOp::AShl => "<<<",
            BinOp::AShr => ">>>",
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Mod => "%",
        };
        write!(f, "{s}")
    }
}