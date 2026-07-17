use crate::ast::{BinOp, UnOp};
use crate::elaboration::{Elaborated, ResolvedExpr, ResolvedStmt};
use std::fmt;

/// ネットリスト中のノードID
pub type NodeId = usize;

/// ネットリストノード
#[derive(Debug, Clone)]
pub enum Node {
    /// 定数
    Const {
        id: NodeId,
        value: u64,
        width: u64,
    },
    /// 信号読み出し
    ReadSignal {
        id: NodeId,
        #[allow(dead_code)]
        signal_id: usize,
        signal_name: String,
        width: u64,
    },
    /// 二項演算
    BinOp {
        id: NodeId,
        op: BinOp,
        lhs: NodeId,
        rhs: NodeId,
        width: u64,
    },
    /// 単項演算
    UnaryOp {
        id: NodeId,
        op: UnOp,
        operand: NodeId,
        width: u64,
    },
    /// 三項演算（条件式）
    Ternary {
        id: NodeId,
        cond: NodeId,
        then_branch: NodeId,
        else_branch: NodeId,
        width: u64,
    },
    /// 信号駆動（組み合わせ）
    Drive {
        id: NodeId,
        #[allow(dead_code)]
        signal_id: usize,
        signal_name: String,
        source: NodeId,
        kind: DriveKind,
        /// 駆動先信号のビット幅（代入時のマスキングに使用）
        width: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DriveKind {
    Combinational,
    Sequential,
}

impl fmt::Display for DriveKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DriveKind::Combinational => write!(f, "blocking"),
            DriveKind::Sequential => write!(f, "non-blocking"),
        }
    }
}

/// クロック/リセットのエッジの向き
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Edge {
    Posedge,
    Negedge,
}

/// reg更新やリセットのトリガーとなる信号とエッジ
#[derive(Debug, Clone, PartialEq)]
pub struct ClockTrigger {
    pub signal_id: usize,
    pub edge: Edge,
}

/// reg のリセット仕様
#[derive(Debug, Clone, PartialEq)]
pub struct ResetSpec {
    pub trigger: ClockTrigger,
    pub value: u64,
}

/// 信号の種別（wire/reg）
///
/// wire/reg宣言構文の先行対応として、regにクロック/リセット情報を持たせられる
/// ようにしている。現状は宣言構文が無いため、`clock`/`reset`は常に`None`
/// （= 既存の`<=`と同じ、ステップ単位での更新という現行の挙動のまま）。
/// `kind`自体は既存の代入演算子（`=`/`<=`）から`build_netlist`が自動的に
/// 決定し、`Combinational`駆動なら`Wire`、`Sequential`駆動なら`Reg`になる。
#[derive(Debug, Clone, PartialEq)]
pub enum SignalKind {
    Wire,
    Reg {
        clock: Option<ClockTrigger>,
        reset: Option<ResetSpec>,
    },
}

/// 生成されたネットリスト
#[derive(Debug, Clone)]
pub struct Netlist {
    pub signals: Vec<NetlistSignal>,
    pub nodes: Vec<Node>,
}

/// ネットリスト上の信号情報
#[derive(Debug, Clone)]
pub struct NetlistSignal {
    pub id: usize,
    pub name: String,
    pub width: u64,
    pub driver_node: Option<NodeId>,
    pub driver_kind: Option<DriveKind>,
    pub kind: SignalKind,
}

/// ネットリストビルダー
pub struct NetlistBuilder {
    nodes: Vec<Node>,
    next_id: NodeId,
}

impl Default for NetlistBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl NetlistBuilder {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            next_id: 0,
        }
    }

    fn alloc_id(&mut self) -> NodeId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn add_node(&mut self, node: Node) -> NodeId {
        let id = match &node {
            Node::Const { id, .. } => *id,
            Node::ReadSignal { id, .. } => *id,
            Node::BinOp { id, .. } => *id,
            Node::UnaryOp { id, .. } => *id,
            Node::Ternary { id, .. } => *id,
            Node::Drive { id, .. } => *id,
        };
        self.nodes.push(node);
        id
    }

    fn make_const(&mut self, value: u64, width: u64) -> NodeId {
        let id = self.alloc_id();
        self.add_node(Node::Const { id, value, width })
    }

    fn make_read_signal(&mut self, signal_id: usize, signal_name: &str, width: u64) -> NodeId {
        let id = self.alloc_id();
        self.add_node(Node::ReadSignal {
            id,
            signal_id,
            signal_name: signal_name.to_string(),
            width,
        })
    }

    fn make_binop(&mut self, op: BinOp, lhs: NodeId, rhs: NodeId, width: u64) -> NodeId {
        let id = self.alloc_id();
        self.add_node(Node::BinOp { id, op, lhs, rhs, width })
    }

    fn make_unaryop(&mut self, op: UnOp, operand: NodeId, width: u64) -> NodeId {
        let id = self.alloc_id();
        self.add_node(Node::UnaryOp { id, op, operand, width })
    }

    fn make_ternary(&mut self, cond: NodeId, then_branch: NodeId, else_branch: NodeId, width: u64) -> NodeId {
        let id = self.alloc_id();
        self.add_node(Node::Ternary { id, cond, then_branch, else_branch, width })
    }

    fn make_drive(
        &mut self,
        signal_id: usize,
        signal_name: &str,
        source: NodeId,
        kind: DriveKind,
        width: u64,
    ) -> NodeId {
        let id = self.alloc_id();
        self.add_node(Node::Drive {
            id,
            signal_id,
            signal_name: signal_name.to_string(),
            source,
            kind,
            width,
        })
    }

    /// 式のノードを構築し、その結果のNodeIdを返す
    fn build_expr(&mut self, expr: &ResolvedExpr, signals: &[NetlistSignal]) -> NodeId {
        match expr {
            ResolvedExpr::Number(n) => {
                // 最小幅を計算（0の場合は1ビット）
                let width = if *n == 0 { 1 } else { 64 - n.leading_zeros() as u64 };
                // ただし最小で1
                let width = width.max(1);
                self.make_const(*n, width)
            }
            ResolvedExpr::BitVecLiteral { width, value } => {
                // 明示された幅に収まらない値は静的な代入と同様に切り詰める（エラーにはしない）
                let masked = if *width >= 64 { *value } else { *value & ((1u64 << width) - 1) };
                self.make_const(masked, *width)
            }
            ResolvedExpr::Ident(signal_id) => {
                let sig = &signals[*signal_id];
                self.make_read_signal(*signal_id, &sig.name, sig.width)
            }
            ResolvedExpr::BinOp { op, lhs, rhs } => {
                let lhs_id = self.build_expr(lhs, signals);
                let rhs_id = self.build_expr(rhs, signals);
                let lhs_width = self.node_width(lhs_id);
                let rhs_width = self.node_width(rhs_id);
                // 論理・比較演算の結果は真偽値（1ビット）、それ以外は両オペランドの大きい方
                let width = match op {
                    BinOp::Or | BinOp::And | BinOp::Eq | BinOp::Neq | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => 1,
                    _ => lhs_width.max(rhs_width),
                };
                self.make_binop(*op, lhs_id, rhs_id, width)
            }
            ResolvedExpr::UnaryOp { op, expr } => {
                let operand_id = self.build_expr(expr, signals);
                // 論理否定の結果は真偽値（1ビット）、ビット反転はオペランドと同じ幅
                let width = match op {
                    UnOp::Not => 1,
                    UnOp::BitNot => self.node_width(operand_id),
                };
                self.make_unaryop(*op, operand_id, width)
            }
            ResolvedExpr::Ternary { cond, then_branch, else_branch } => {
                let cond_id = self.build_expr(cond, signals);
                let then_id = self.build_expr(then_branch, signals);
                let else_id = self.build_expr(else_branch, signals);
                // 結果の幅はthen/elseの大きい方（condは選択にのみ使い、幅には影響しない）
                let width = self.node_width(then_id).max(self.node_width(else_id));
                self.make_ternary(cond_id, then_id, else_id, width)
            }
        }
    }

    fn node_width(&self, node_id: NodeId) -> u64 {
        match &self.nodes[node_id] {
            Node::Const { width, .. } => *width,
            Node::ReadSignal { width, .. } => *width,
            Node::BinOp { width, .. } => *width,
            Node::UnaryOp { width, .. } => *width,
            Node::Ternary { width, .. } => *width,
            Node::Drive { width, .. } => *width,
        }
    }
}

/// エラボレーション結果からネットリストを生成する
pub fn build_netlist(elab: &Elaborated) -> Netlist {
    // 信号リストを作成
    let mut signals: Vec<NetlistSignal> = elab
        .signals
        .iter()
        .map(|s| NetlistSignal {
            id: s.id,
            name: s.name.clone(),
            width: s.width,
            driver_node: None,
            driver_kind: None,
            kind: SignalKind::Wire,
        })
        .collect();

    let mut builder = NetlistBuilder::new();

    // 各文からノード構築
    for stmt in &elab.stmts {
        match stmt {
            ResolvedStmt::Combinational { target_id, expr } => {
                let sig = &signals[*target_id];
                let src = builder.build_expr(expr, &signals);
                let drive = builder.make_drive(*target_id, &sig.name, src, DriveKind::Combinational, sig.width);
                signals[*target_id].driver_node = Some(drive);
                signals[*target_id].driver_kind = Some(DriveKind::Combinational);
            }
            ResolvedStmt::Sequential { target_id, expr } => {
                let sig = &signals[*target_id];
                let src = builder.build_expr(expr, &signals);
                let drive = builder.make_drive(*target_id, &sig.name, src, DriveKind::Sequential, sig.width);
                signals[*target_id].driver_node = Some(drive);
                signals[*target_id].driver_kind = Some(DriveKind::Sequential);
                signals[*target_id].kind = SignalKind::Reg { clock: None, reset: None };
            }
        }
    }

    Netlist {
        signals,
        nodes: builder.nodes,
    }
}

/// ネットリストをテキスト出力
pub fn format_netlist(nl: &Netlist) -> String {
    let mut out = String::new();

    out.push_str("===== Netlist =====\n\n");

    out.push_str("--- Signals ---\n");
    for sig in &nl.signals {
        let driver = match (&sig.driver_kind, &sig.driver_node) {
            (Some(kind), Some(node_id)) => format!("  driven by N{} ({})", node_id, kind),
            _ => "  (no driver)".into(),
        };
        out.push_str(&format!(
            "  {}[{}:0] : bit{}  (id={})\n",
            sig.name,
            sig.width - 1,
            if sig.width > 1 { format!("[{}]", sig.width) } else { String::new() },
            sig.id,
        ));
        out.push_str(&format!("             {}\n", driver));
    }

    out.push_str("\n--- Nodes ---\n");
    for node in &nl.nodes {
        match node {
            Node::Const { id, value, width } => {
                out.push_str(&format!("  N{:>3}: Const({})  ({} bit)\n", id, value, width));
            }
            Node::ReadSignal { id, signal_name, width, .. } => {
                out.push_str(&format!("  N{:>3}: Read({})  ({} bit)\n", id, signal_name, width));
            }
            Node::BinOp { id, op, lhs, rhs, width } => {
                out.push_str(&format!(
                    "  N{:>3}: BinOp({})  ({} bit)  = N{} {} N{}\n",
                    id, op, width, lhs, op, rhs,
                ));
            }
            Node::UnaryOp { id, op, operand, width } => {
                out.push_str(&format!(
                    "  N{:>3}: UnaryOp({})  ({} bit)  = {}N{}\n",
                    id, op, width, op, operand,
                ));
            }
            Node::Ternary { id, cond, then_branch, else_branch, width } => {
                out.push_str(&format!(
                    "  N{:>3}: Ternary  ({} bit)  = N{} ? N{} : N{}\n",
                    id, width, cond, then_branch, else_branch,
                ));
            }
            Node::Drive { id, signal_name, source, kind, .. } => {
                out.push_str(&format!(
                    "  N{:>3}: Drive({})  ({})  <= N{}\n",
                    id, signal_name, kind, source,
                ));
            }
        }
    }

    out
}