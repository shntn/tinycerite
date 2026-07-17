use std::fmt;

/// プログラム全体（トップレベルのブロック・モジュール定義・テストベンチ定義）
#[derive(Debug, Clone)]
pub struct Program {
    pub blocks: Vec<Block>,
    pub modules: Vec<ModuleDef>,
    pub testbenches: Vec<TestbenchDef>,
}

/// ブロック
#[derive(Debug, Clone)]
pub struct Block {
    pub decls: Vec<Decl>,
    pub instances: Vec<InstDecl>,
    pub stmts: Vec<Stmt>,
}

/// ポートの向き
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Direction {
    Input,
    Output,
}

/// 信号の型（`bit`/`bit<N>`/`clock`）
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SignalType {
    /// `None` = 1-bit（`bit`）、`Some(n)` = nビット（`bit<n>`）
    Bit(Option<u64>),
    /// クロック信号。常に1ビット扱いで`<N>`は書けない
    Clock,
}

impl SignalType {
    pub fn width(&self) -> u64 {
        match self {
            SignalType::Bit(w) => w.unwrap_or(1),
            SignalType::Clock => 1,
        }
    }

    pub fn is_clock(&self) -> bool {
        matches!(self, SignalType::Clock)
    }
}

/// ポート宣言 `name: input/output bit<N>;` / `name: input/output clock;`
#[derive(Debug, Clone)]
pub struct PortDecl {
    pub name: String,
    pub direction: Direction,
    pub sig_type: SignalType,
}

/// モジュール定義
#[derive(Debug, Clone)]
pub struct ModuleDef {
    pub name: String,
    pub ports: Vec<PortDecl>,
    pub decls: Vec<Decl>,
    pub stmts: Vec<Stmt>,
}

/// モジュールインスタンス化宣言 `var name = module_name(port: expr, ...);`
#[derive(Debug, Clone)]
pub struct InstDecl {
    pub instance_name: String,
    pub module_name: String,
    pub args: Vec<(String, Expr)>,
}

/// テストベンチ定義。プログラム中に高々1つ。
///
/// `decls`/`instances`/`stmts` は常時並行に動く既存の代入文と同じ意味論で
/// トップレベルの信号空間に合流する。`initial` だけが手続き的（上から順に
/// 実行され、`step`で明示的に1サイクル進める）という別の意味論を持つ。
#[derive(Debug, Clone)]
pub struct TestbenchDef {
    pub name: String,
    pub decls: Vec<Decl>,
    pub instances: Vec<InstDecl>,
    pub stmts: Vec<Stmt>,
    pub initial: Vec<ProcStmt>,
}

/// `initial { }` 内の手続き文
#[derive(Debug, Clone)]
pub enum ProcStmt {
    /// `target = expr;` — その瞬間に一度だけ値を設定する（継続的な駆動ではない）
    Assign {
        target: String,
        expr: Expr,
    },
    /// `step;` — シミュレーションを1サイクル進める
    Step,
}

/// 変数宣言
#[derive(Debug, Clone)]
pub struct Decl {
    pub name: String,
    pub sig_type: SignalType,
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
    /// モジュールインスタンスの出力ポート参照 `instance.field`
    FieldAccess {
        instance: String,
        field: String,
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