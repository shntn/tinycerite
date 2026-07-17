use crate::netlist::{DriveKind, Node, NodeId};

/// 組み合わせ評価の最大Δ-サイクル数（これを超えると組合せループと判定）
const MAX_COMB_ITERATIONS: u32 = 1000;

/// シミュレーション結果の1サイクル分
#[derive(Debug, Clone)]
pub struct CycleSnapshot {
    pub cycle: u64,
    pub values: Vec<u64>,
}

/// シミュレーター
pub struct Simulator {
    signal_values: Vec<u64>,
    cycle: u64,
}

impl Simulator {
    /// 全信号を0で初期化
    pub fn new(signal_count: usize) -> Self {
        Self {
            signal_values: vec![0; signal_count],
            cycle: 0,
        }
    }

    /// 信号に初期値を設定（0サイクル目実行前）
    pub fn set_signal(&mut self, id: usize, value: u64) {
        self.signal_values[id] = value;
    }

    /// 現在の信号値を取得
    pub fn signal_values(&self) -> &[u64] {
        &self.signal_values
    }

    /// 現在のサイクル数
    pub fn cycle(&self) -> u64 {
        self.cycle
    }

    /// 1サイクル進める
    pub fn step(&mut self, nodes: &[Node]) -> CycleSnapshot {
        // サイクル開始時の値でスナップショット（ノンブロッキング代入の参照用）
        let snapshot = self.signal_values.clone();

        // Phase 1: 組み合わせ評価（Δ-サイクル収束まで、上限付き）
        let mut converged = false;
        for _ in 0..MAX_COMB_ITERATIONS {
            let old = self.signal_values.clone();
            for node in nodes {
                if let Node::Drive {
                    signal_id,
                    source,
                    kind: DriveKind::Combinational,
                    width,
                    ..
                } = node
                {
                    let value = eval_node(*source, nodes, &self.signal_values);
                    self.signal_values[*signal_id] = mask_to_width(value, *width);
                }
            }
            if old == self.signal_values {
                converged = true;
                break;
            }
        }
        if !converged {
            panic!(
                "組合せループを検出: {}回のΔ反復で収束しませんでした。\
                 回路に組合せフィードバック（例: a = a ^ 1）がないか確認してください",
                MAX_COMB_ITERATIONS
            );
        }

        // Phase 2: 順序評価（ノンブロッキング）
        // 参照する値はサイクル開始時の snapshot（comb による更新前）
        let mut next = self.signal_values.clone();
        for node in nodes {
            if let Node::Drive {
                signal_id,
                source,
                kind: DriveKind::Sequential,
                width,
                ..
            } = node
            {
                let value = eval_node(*source, nodes, &snapshot);
                next[*signal_id] = mask_to_width(value, *width);
            }
        }
        self.signal_values = next;

        let snapshot_out = CycleSnapshot {
            cycle: self.cycle,
            values: self.signal_values.clone(),
        };
        self.cycle += 1;
        snapshot_out
    }

    /// Nサイクル実行し、全スナップショットを返す
    pub fn run(&mut self, nodes: &[Node], cycles: u64) -> Vec<CycleSnapshot> {
        (0..cycles).map(|_| self.step(nodes)).collect()
    }
}

/// 値を信号のビット幅に切り詰める（幅が64以上ならそのまま）
fn mask_to_width(value: u64, width: u64) -> u64 {
    if width >= 64 {
        value
    } else {
        value & ((1u64 << width) - 1)
    }
}

/// ノードの値を再帰評価する
fn eval_node(node_id: NodeId, nodes: &[Node], signal_values: &[u64]) -> u64 {
    match &nodes[node_id] {
        Node::Const { value, .. } => *value,
        Node::ReadSignal { signal_id, .. } => signal_values[*signal_id],
        Node::BinOp { op, lhs, rhs, .. } => {
            let l = eval_node(*lhs, nodes, signal_values);
            let r = eval_node(*rhs, nodes, signal_values);
            eval_binop(*op, l, r)
        }
        Node::UnaryOp { op, operand, .. } => {
            let v = eval_node(*operand, nodes, signal_values);
            eval_unaryop(*op, v)
        }
        Node::Drive { source, .. } => eval_node(*source, nodes, signal_values),
    }
}

/// 二項演算子を適用する
///
/// シフト量が64以上の場合と0除算は、この言語に未定義値('x')が無いため0を返す。
/// 加減乗算はここでは幅マスキングを行わずu64のラップアラウンドで近似する。
/// 信号への代入時にのみ mask_to_width で宣言幅に切り詰められる。
fn eval_binop(op: crate::ast::BinOp, l: u64, r: u64) -> u64 {
    use crate::ast::BinOp;
    match op {
        BinOp::Or => u64::from(l != 0 || r != 0),
        BinOp::And => u64::from(l != 0 && r != 0),
        BinOp::BitOr => l | r,
        BinOp::Xor => l ^ r,
        BinOp::BitAnd => l & r,
        BinOp::Eq => u64::from(l == r),
        BinOp::Neq => u64::from(l != r),
        BinOp::Lt => u64::from(l < r),
        BinOp::Le => u64::from(l <= r),
        BinOp::Gt => u64::from(l > r),
        BinOp::Ge => u64::from(l >= r),
        BinOp::Shl | BinOp::AShl => l.checked_shl(r as u32).unwrap_or(0),
        BinOp::Shr | BinOp::AShr => l.checked_shr(r as u32).unwrap_or(0),
        BinOp::Add => l.wrapping_add(r),
        BinOp::Sub => l.wrapping_sub(r),
        BinOp::Mul => l.wrapping_mul(r),
        BinOp::Div => l.checked_div(r).unwrap_or(0),
        BinOp::Mod => l.checked_rem(r).unwrap_or(0),
    }
}

/// 単項演算子を適用する
///
/// `~` はここでは幅マスキングを行わずu64のビット反転で近似する（他の演算と同様、
/// 信号への代入時にのみ mask_to_width で宣言幅に切り詰められる）。
fn eval_unaryop(op: crate::ast::UnOp, v: u64) -> u64 {
    use crate::ast::UnOp;
    match op {
        UnOp::Not => u64::from(v == 0),
        UnOp::BitNot => !v,
    }
}

/// シミュレーション結果をテキスト波形として整形
pub fn format_waveform(snapshots: &[CycleSnapshot], signals: &[crate::netlist::NetlistSignal]) -> String {
    if snapshots.is_empty() || signals.is_empty() {
        return String::new();
    }

    let mut out = String::new();

    // ヘッダー行
    out.push_str("cycle");
    for sig in signals {
        out.push_str(&format!(" {:>width$}", sig.name, width = sig.width.max(2) as usize));
    }
    out.push('\n');

    // 区切り線
    out.push_str("-----");
    for sig in signals {
        out.push_str(&"-".repeat(sig.width.max(2) as usize + 1));
    }
    out.push('\n');

    // 各行
    for snap in snapshots {
        out.push_str(&format!("{:>5}", snap.cycle));
        for sig in signals {
            let val = snap.values.get(sig.id).copied().unwrap_or(0);
            let w = sig.width.max(2) as usize;
            out.push_str(&format!(" {:>width$}", val, width = w));
        }
        out.push('\n');
    }

    out
}