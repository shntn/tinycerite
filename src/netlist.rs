use crate::ast::{BinOp, Direction, UnOp};
use crate::elaboration::{Elaborated, ResolvedExpr, ResolvedModuleDef, ResolvedProcStmt, ResolvedScope, ResolvedStmt};
use std::collections::HashMap;
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
/// `reg`/`wire`を区別する宣言キーワードは無く、`kind`は既存の代入演算子
/// （`=`/`<=`）から`build_netlist`が自動的に決定する（`Combinational`駆動なら
/// `Wire`、`Sequential`駆動なら`Reg`）。`Reg`の`clock`は、そのregがモジュール
/// 本体に属し、かつそのモジュールに`clock`型入力ポートがあれば`Some`
/// （トップレベル/テストベンチ直下のregはモジュールに属さないため常に`None`）。
/// `reset`は現状常に`None`（先行実装のみで未使用）。
#[derive(Debug, Clone, PartialEq)]
pub enum SignalKind {
    Wire,
    Reg {
        clock: Option<ClockTrigger>,
        reset: Option<ResetSpec>,
    },
}

/// `initial { }` の手続き文をネットリスト向けに展開したもの
#[derive(Debug, Clone)]
pub enum InitialStep {
    /// 対象信号(グローバルID)を、式ノードを評価した値で即座に設定する（継続的な駆動ではない）
    Assign { target: usize, expr_node: NodeId },
    /// シミュレーションを1サイクル進める
    Step,
}

/// 生成されたネットリスト
#[derive(Debug, Clone)]
pub struct Netlist {
    pub signals: Vec<NetlistSignal>,
    pub nodes: Vec<Node>,
    pub initial: Vec<InitialStep>,
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

/// スコープ内のインスタンス名 → (モジュール名, ローカル信号ID→グローバル信号IDのリマップ)
type InstanceRemaps = HashMap<String, (String, Vec<usize>)>;

/// ネットリストビルダー
///
/// `flatten_scope` がモジュール階層を再帰的に辿り、各スコープの信号にインスタンス名の
/// プレフィックスを付けてフラットな `signals`/`nodes` へ展開する。展開が終われば
/// `Node`/`NetlistSignal` はモジュールの存在を一切知らないフラットなDAGになる。
pub struct NetlistBuilder {
    nodes: Vec<Node>,
    signals: Vec<NetlistSignal>,
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
            signals: Vec::new(),
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

    /// スコープ（トップレベルまたはモジュール本体）をフラットな信号・ノードへ展開する
    ///
    /// `prefix`は展開後の信号名に付ける名前空間（トップレベルは空文字列、インスタンスは
    /// `"u1"`のようなインスタンス名）。`clock_port`は、このスコープがモジュール本体の場合に
    /// そのモジュールの`clock`型入力ポートのローカル信号ID（エラボレーションで高々1つに
    /// 限定済み）、トップレベルの場合は常に`None`。regの`SignalKind::Reg.clock`を
    /// 決定するのに使う（モジュールの外のreg、例えばテストベンチの`counter <= counter + 1;`
    /// のようなものは、モジュールに属さないためクロックを持たない）。
    /// 戻り値はこのスコープのローカル信号ID→グローバル信号IDのリマップと、このスコープが
    /// 直接持つインスタンスのリマップ（呼び出し元がポート接続や、トップレベルなら`initial`の
    /// 式構築に使う）。
    fn flatten_scope(
        &mut self,
        scope: &ResolvedScope,
        prefix: &str,
        modules: &HashMap<String, ResolvedModuleDef>,
        clock_port: Option<usize>,
    ) -> (Vec<usize>, InstanceRemaps) {
        let base = self.signals.len();
        for sig in &scope.signals {
            self.signals.push(NetlistSignal {
                id: base + sig.id,
                name: scoped_name(prefix, &sig.name),
                width: sig.width,
                driver_node: None,
                driver_kind: None,
                kind: SignalKind::Wire,
            });
        }
        let remap: Vec<usize> = (0..scope.signals.len()).map(|local| base + local).collect();

        let mut instance_remaps: InstanceRemaps = HashMap::new();
        for inst in &scope.instances {
            let module_def = &modules[&inst.module_name];
            let inst_prefix = scoped_name(prefix, &inst.instance_name);
            let (inst_remap, _) = self.flatten_scope(&module_def.body, &inst_prefix, modules, module_def.clock_port);

            // 入力ポートへの接続を、外側スコープの式から合成の組み合わせDriveとして生成する
            for port in module_def.ports.iter().filter(|p| p.direction == Direction::Input) {
                let conn_expr = &inst.connections[&port.name];
                let src = self.build_expr(conn_expr, &remap, &instance_remaps, modules);
                self.drive_signal(inst_remap[port.signal_id], src, DriveKind::Combinational);
            }

            instance_remaps.insert(inst.instance_name.clone(), (inst.module_name.clone(), inst_remap));
        }

        for stmt in &scope.stmts {
            match stmt {
                ResolvedStmt::Combinational { target_id, expr } => {
                    let src = self.build_expr(expr, &remap, &instance_remaps, modules);
                    self.drive_signal(remap[*target_id], src, DriveKind::Combinational);
                }
                ResolvedStmt::Sequential { target_id, expr } => {
                    let src = self.build_expr(expr, &remap, &instance_remaps, modules);
                    let target_global = remap[*target_id];
                    self.drive_signal(target_global, src, DriveKind::Sequential);
                    let clock = clock_port.map(|local_id| ClockTrigger {
                        signal_id: remap[local_id],
                        // TODO: 暫定的にposedge固定。negedge/両エッジのサポートは未実装のため、
                        // clock型入力ポートを持つモジュールのregは常にposedgeトリガーとして扱う。
                        edge: Edge::Posedge,
                    });
                    self.signals[target_global].kind = SignalKind::Reg { clock, reset: None };
                }
            }
        }

        (remap, instance_remaps)
    }

    /// 信号をDriveノードで駆動し、`NetlistSignal`の駆動情報を更新する
    fn drive_signal(&mut self, target_global: usize, source: NodeId, kind: DriveKind) {
        let name = self.signals[target_global].name.clone();
        let width = self.signals[target_global].width;
        let drive = self.make_drive(target_global, &name, source, kind, width);
        self.signals[target_global].driver_node = Some(drive);
        self.signals[target_global].driver_kind = Some(kind);
    }

    /// 式のノードを構築し、その結果のNodeIdを返す
    ///
    /// `remap`は現在のスコープのローカル信号ID→グローバル信号IDのリマップ、
    /// `instance_remaps`は現在のスコープが直接持つインスタンスのリマップ（`InstanceField`の解決に使う）。
    fn build_expr(
        &mut self,
        expr: &ResolvedExpr,
        remap: &[usize],
        instance_remaps: &InstanceRemaps,
        modules: &HashMap<String, ResolvedModuleDef>,
    ) -> NodeId {
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
                let global = remap[*signal_id];
                let name = self.signals[global].name.clone();
                let width = self.signals[global].width;
                self.make_read_signal(global, &name, width)
            }
            ResolvedExpr::InstanceField { instance_name, port_name } => {
                let (module_name, inst_remap) = &instance_remaps[instance_name];
                let module_def = &modules[module_name];
                let port = module_def
                    .ports
                    .iter()
                    .find(|p| &p.name == port_name)
                    .expect("解決済みのInstanceFieldは常に実在するポートを参照する");
                let global = inst_remap[port.signal_id];
                let name = self.signals[global].name.clone();
                let width = self.signals[global].width;
                self.make_read_signal(global, &name, width)
            }
            ResolvedExpr::BinOp { op, lhs, rhs } => {
                let lhs_id = self.build_expr(lhs, remap, instance_remaps, modules);
                let rhs_id = self.build_expr(rhs, remap, instance_remaps, modules);
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
                let operand_id = self.build_expr(expr, remap, instance_remaps, modules);
                // 論理否定の結果は真偽値（1ビット）、ビット反転はオペランドと同じ幅
                let width = match op {
                    UnOp::Not => 1,
                    UnOp::BitNot => self.node_width(operand_id),
                };
                self.make_unaryop(*op, operand_id, width)
            }
            ResolvedExpr::Ternary { cond, then_branch, else_branch } => {
                let cond_id = self.build_expr(cond, remap, instance_remaps, modules);
                let then_id = self.build_expr(then_branch, remap, instance_remaps, modules);
                let else_id = self.build_expr(else_branch, remap, instance_remaps, modules);
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

/// 信号名に名前空間プレフィックスを付ける（トップレベルはプレフィックス無し）
fn scoped_name(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}.{name}")
    }
}

/// エラボレーション結果からネットリストを生成する
///
/// モジュール階層はここで再帰的にフラット化される（`NetlistBuilder::flatten_scope`）。
/// 展開後の`Node`/`NetlistSignal`はモジュールの存在を知らないため、`Simulator`は
/// 今まで通り変更不要。
pub fn build_netlist(elab: &Elaborated) -> Netlist {
    let mut builder = NetlistBuilder::new();
    let (remap, instance_remaps) = builder.flatten_scope(&elab.top, "", &elab.modules, None);

    let mut initial = Vec::new();
    for step in &elab.initial {
        let step = match step {
            ResolvedProcStmt::Assign { target_id, expr } => {
                let expr_node = builder.build_expr(expr, &remap, &instance_remaps, &elab.modules);
                InitialStep::Assign { target: remap[*target_id], expr_node }
            }
            ResolvedProcStmt::Step => InitialStep::Step,
        };
        initial.push(step);
    }

    Netlist {
        signals: builder.signals,
        nodes: builder.nodes,
        initial,
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
