# Tiny Cerilte — アーキテクチャドキュメント

tinycerilte は最小の HDL シミュレータ。入出力は以下。

```
Source (.tc)
  ↓ [Phase 1] Lexer
Token列
  ↓ [Phase 2] Parser
AST (Program)
  ↓ [Phase 3] Elaboration
解決済みIR (Elaborated)
  ↓ [Phase 4] Netlist Builder
Netlist (信号DAG)
  ↓ [Phase 5] Simulator
波形出力
```

---

## 言語仕様 (Language)

### 文法 (EBNF)

```
program     = block+
block       = "{" (decl | stmt)* "}"
decl        = "var" ident ":" "bit" ("<" number ">")? ";"
stmt        = ident ("=" | "<=") expr ";"
expr        = primary ("^" primary)?
primary     = ident | number
ident       = [a-zA-Z_][a-zA-Z0-9_]*
number      = [0-9]+
```

### セマンティクス

- `var x: bit` — 1ビットの信号 x を宣言（初期値 0）
- `var x: bit<N>` — Nビットの信号 x を宣言
- `a = expr;` — 組み合わせ代入（即時反映）
- `a <= expr;` — 順序代入（サイクル開始時の値で評価、サイクル終了時に一斉反映）
- `a ^ b` — ビット単位 XOR

### サンプル

```
{
    var a: bit;
    var b: bit;

    a = b ^ 1;    // 組み合わせ: a は b の反転
    b <= a;       // 順序: b は a を1サイクル遅れで追従
}
```

この回路はトグルフリップフロップ（TFF）として動作し、a は2サイクルごとに 1→0 を繰り返す。

---

## モジュール一覧

| モジュール | ファイル | 役割 |
|---|---|---|
| `ast` | `src/ast.rs` | AST 型定義 |
| `lexer` | `src/lexer.rs` | 字句解析（Source → Token） |
| `parser` | `src/parser.rs` | 構文解析（Token → AST） |
| `elaboration` | `src/elaboration.rs` | シンボル解決・型解決 |
| `netlist` | `src/netlist.rs` | ネットリスト生成（DAG構築） |
| `simulator` | `src/simulator.rs` | シミュレーション実行 |
| `main` | `src/main.rs` | CLI エントリポイント |

---

## データ構造詳細

### `ast` モジュール

#### `Program`
- 役割: パース結果のトップレベル。0個以上の Block を持つ。
- フィールド:
  - `blocks: Vec<Block>` — プログラム中のブロックのリスト

#### `Block`
- 役割: `{ ... }` で囲まれた1つのスコープ。宣言と代入文の列。
- フィールド:
  - `decls: Vec<Decl>` — 変数宣言のリスト
  - `stmts: Vec<Stmt>` — 代入文のリスト

#### `Decl`
- 役割: `var name: bit<N>;` による変数宣言。
- フィールド:
  - `name: String` — 変数名
  - `width: Option<u64>` — ビット幅。`None` = `bit`（1ビット）、`Some(n)` = `bit<n>`（Nビット）

#### `Stmt` (enum)
- 役割: 代入文。代入演算子でバリアントが分かれる。
- バリアント:
  - `Combinational { target: String, expr: Expr }` — `target = expr`
    - `target` — 代入先の変数名
    - `expr` — 右辺の式
  - `Sequential { target: String, expr: Expr }` — `target <= expr`
    - `target` — 代入先の変数名
    - `expr` — 右辺の式

- メソッド:
  - `target(&self) -> &str` — 代入先の変数名を返す
  - `expr(&self) -> &Expr` — 右辺の式への参照を返す

#### `Expr` (enum)
- 役割: 式。右辺の計算を表す木構造。
- バリアント:
  - `Ident(String)` — 変数参照（例: `a`）
  - `Number(u64)` — 10進数リテラル（例: `1`、`42`）
  - `BinOp { op: BinOp, lhs: Box<Expr>, rhs: Box<Expr> }` — 二項演算
    - `op` — 演算子の種類
    - `lhs` — 左辺の式
    - `rhs` — 右辺の式

#### `BinOp` (enum)
- 役割: 二項演算子の種類。
- バリアント:
  - `Xor` — ビット単位 XOR（`^`）

- `Display` 実装: `Xor` → `"^"`

---

### `lexer` モジュール

#### `Token` (enum)
- 役割: 字句解析の出力単位。1つの「単語」を表す。
- バリアント:
  - `Var` — キーワード `var`
  - `Bit` — キーワード `bit`
  - `LBrace` — `{`
  - `RBrace` — `}`
  - `LBracket` — `[`
  - `RBracket` — `]`
  - `LAngle` — `<`
  - `RAngle` — `>`
  - `Semicolon` — `;`
  - `Colon` — `:`
  - `Assign` — `=`
  - `NonBlockAssign` — `<=`
  - `Caret` — `^`
  - `Number(u64)` — 数値リテラル（10進数）
  - `Ident(String)` — 識別子（変数名）
  - `Eof` — 入力終端
  - `Error(String)` — 字句エラー（内容を文字列で保持）

- `Display` 実装: 各トークンに対応する文字列表現を返す（エラーは `<ERROR: ...>`）

#### `Lexer`
- 役割: ソース文字列を先頭から読み、1トークンずつ切り出す。
- フィールド:
  - `chars: Vec<char>` — 入力文字列を char の配列にしたもの
  - `pos: usize` — 現在の読み取り位置（0-indexed）

##### `Lexer::new(input: &str) -> Self`
- 概要: 入力文字列を受け取り、Lexer を初期化する。
- 引数: `input` — パース対象のソースコード文字列
- 処理: 入力を `Vec<char>` に変換し、位置を0にセットする。

##### `Lexer::next_token(&mut self) -> Token`
- 概要: 現在位置から次の1トークンを読み、位置を進めて返す。
- 処理:
  1. `skip_whitespace()` で空白を読み飛ばす
  2. 位置が末尾なら `Eof` を返す
  3. 先頭文字が英字または `_` → `read_ident_or_keyword()`
  4. 先頭文字が数字 → `read_number()`
  5. 記号の場合:
     - `{` `}` `[` `]` `;` `:` `^` `=` → 対応するトークンを返す
     - `<` → 次が `=` なら `NonBlockAssign`、さもなくば `LAngle`
     - `>` → `RAngle`
     - その他 → `Error`

##### `Lexer::read_ident_or_keyword(&mut self) -> Token`
- 概要: 英数字と `_` の連続を読み、キーワードか識別子かを判定する。
- 処理: `var` → `Var`, `bit` → `Bit`, それ以外 → `Ident(word)`

##### `Lexer::read_number(&mut self) -> Token`
- 概要: 数字の連続を読み、`u64` にパースする。
- 処理: パース成功 → `Number(n)`, 失敗 → `Error`

##### `Lexer::skip_whitespace(&mut self)`
- 概要: 空白文字（ASCII スペース、タブ、改行）をスキップする。

---

### `parser` モジュール

#### `ParseError`
- 役割: 構文解析エラー。
- フィールド:
  - `message: String` — エラーメッセージ（日本語）

- `Display` 実装: `"パースエラー: <message>"`

#### `Parser`
- 役割: 再帰下降パーサー。Token 列から AST を構築する。
- フィールド:
  - `lexer: Lexer` — 内部で持つ字句解析器
  - `current: Token` — 現在注目中のトークン（1文字先読み）

##### `Parser::new(input: &str) -> Self`
- 概要: 入力文字列から Parser を初期化し、最初のトークンを読み込む。

##### `Parser::parse_program(&mut self) -> Result<Program>`
- 概要: `Program := Block+`
- 処理: `Eof` が来るまで `parse_block()` を繰り返し、`Program` を返す。

##### `Parser::parse_block(&mut self) -> Result<Block>`
- 概要: `Block := "{" (Decl | Stmt)* "}"`
- 処理:
  1. `{` を期待
  2. `}` か `Eof` が来るまで、`var` なら `parse_decl()`、それ以外なら `parse_stmt()` を呼ぶ
  3. `}` を期待して `Block` を返す

##### `Parser::parse_decl(&mut self) -> Result<Decl>`
- 概要: `Decl := "var" Ident ":" "bit" ("<" Number ">")? ";"`
- 処理:
  1. `var` を期待
  2. 識別子を読む（変数名）
  3. `:` を期待
  4. `bit` を期待
  5. `<` があれば数値を読んで幅として保持、なければ `None`
  6. `>` を期待（幅ありの場合）
  7. `;` を期待して `Decl` を返す

##### `Parser::parse_stmt(&mut self) -> Result<Stmt>`
- 概要: `Stmt := Ident ("=" | "<=") Expr ";"`
- 処理:
  1. 識別子を読む（代入先）
  2. `=` なら `Combinational`、`<=` なら `Sequential` を生成
  3. 右辺を `parse_expr()` で読む
  4. `;` を期待して `Stmt` を返す

##### `Parser::parse_expr(&mut self) -> Result<Expr>`
- 概要: `Expr := Primary ("^" Primary)?`
- 処理:
  1. `parse_primary()` で左辺を読む
  2. `^` があれば右辺も読んで `BinOp::Xor` を返す
  3. なければ左辺をそのまま返す

##### `Parser::parse_primary(&mut self) -> Result<Expr>`
- 概要: `Primary := Ident | Number`
- 処理: 現在のトークンが識別子なら `Ident(name)`、数値なら `Number(n)` を返す。それ以外はエラー。

##### `Parser::check(&self, expected: &Token) -> bool`
- 概要: 現在のトークンが期待した種類か判定する（値は比較せず discriminant のみ）。値の検証は `expect_ident` / `expect_number` で行う。

##### `Parser::advance(&mut self)`
- 概要: 次のトークンを読み込む。

##### `Parser::expect(&mut self, expected: &Token) -> Result<()>`
- 概要: 現在のトークンが期待した種類なら読み進め、違えばエラーを返す。

##### `Parser::expect_ident(&mut self) -> Result<String>`
- 概要: 現在のトークンが識別子ならその名前を返し、違えばエラー。

##### `Parser::expect_number(&mut self) -> Result<u64>`
- 概要: 現在のトークンが数値ならその値を返し、違えばエラー。

---

### `elaboration` モジュール

#### `ElabError`
- 役割: エラボレーションエラー（未宣言変数、重複宣言など）。
- フィールド:
  - `message: String` — 日本語のエラーメッセージ

#### `ResolvedSignal`
- 役割: 解決済みの信号定義。パース時の `Decl` から変数名をシンボルテーブルで ID に変換したもの。
- フィールド:
  - `name: String` — 変数名（元のソースの名前）
  - `width: u64` — ビット幅（`bit` = 1、`bit<N>` = N）
  - `id: usize` — 信号ID（0始まりの通番）

#### `ResolvedStmt` (enum)
- 役割: 解決済みの代入文。変数名が ID に置き換わっている。
- バリアント:
  - `Combinational { target_id: usize, expr: ResolvedExpr }` — 組み合わせ代入
    - `target_id` — 代入先信号のID
    - `expr` — 右辺の解決済み式
  - `Sequential { target_id: usize, expr: ResolvedExpr }` — 順序代入

#### `ResolvedExpr` (enum)
- 役割: 解決済みの式。変数参照が ID に置き換わっている。
- バリアント:
  - `Ident(usize)` — 信号ID参照
  - `Number(u64)` — 数値リテラル
  - `BinOp { op: BinOp, lhs: Box<ResolvedExpr>, rhs: Box<ResolvedExpr> }` — 二項演算

#### `Elaborated`
- 役割: エラボレーション結果全体。
- フィールド:
  - `signals: Vec<ResolvedSignal>` — 全信号のリスト
  - `stmts: Vec<ResolvedStmt>` — 全代入文の解決後リスト

#### `SymbolTable` (type alias)
- 定義: `HashMap<String, usize>`
- 役割: 変数名 → 信号ID のマッピング。エラボレーション中に一時的に構築される。

#### `elaborate(prog: &Program) -> Result<Elaborated>`
- 概要: AST を受け取り、シンボル解決と型解決を行い、解決済みIR を返す。
- 処理:
  1. Phase 1 — 宣言走査:
     - 全ブロックの全宣言を走査
     - 重複チェック（同名変数があればエラー）
     - シンボルテーブル（名前→ID）を構築
     - `ResolvedSignal` のリストを作成（`width` のデフォルトは1）
  2. Phase 2 — 文解決（多重ドライバチェック付き）:
     - 全ブロックの全文を走査
     - 代入先の変数名をシンボルテーブルで ID に解決（未宣言ならエラー）
     - `HashSet` で同一信号への複数代入を検出（あればエラー）
     - 右辺の式を再帰的に解決（`resolve_expr`）
     - 代入の種類（Combinational/Sequential）を保持
  3. Phase 3 — 組合せループ検出:
     - `check_combinational_loops` を呼び、組合せ代入間の循環依存を検出（あればエラー）

#### `resolve_expr(expr: &Expr, symtab: &SymbolTable) -> Result<ResolvedExpr>`
- 概要: AST の式を再帰的に解決済み式に変換する。
- 処理:
  - `Ident(name)` → シンボルテーブルで ID に解決
  - `Number(n)` → そのまま
  - `BinOp { op, lhs, rhs }` → 左右を再帰解決して `ResolvedExpr::BinOp`

#### `check_combinational_loops(stmts: &[ResolvedStmt], signal_names: &[String]) -> Result<()>`
- 概要: 組合せ代入（Combinational）だけを対象に依存グラフを作り、循環がないか検査する。順序代入（Sequential）は1サイクル遅れて反映されるため依存グラフに含めない（循環があってもループにならない）。
- 処理:
  1. `deps[信号ID] = その信号を右辺で読む Combinational Drive のターゲットID一覧` を構築（`collect_read_signals` で各文の右辺から読み取り信号を収集）
  2. 白（未訪問）・灰（探索中）・黒（探索済み）で色付けしながら DFS
  3. 探索中（灰）のノードに戻る辺を見つけたら循環と判定し、経路を含めたエラーメッセージを返す
  4. 全信号について探索し終えれば `Ok(())`

#### `collect_read_signals(expr: &ResolvedExpr) -> Vec<usize>`
- 概要: 解決済み式が右辺で参照している信号IDを再帰的に集める（`check_combinational_loops` の依存グラフ構築に使用）。
- 処理:
  - `Ident(id)` → `[id]`
  - `Number(_)` → 空
  - `BinOp { lhs, rhs, .. }` → 左右を再帰収集して連結

---

### `netlist` モジュール

#### `NodeId` (type alias)
- 定義: `usize`
- 役割: ネットリストノードの識別子（`nodes` ベクタのインデックス）

#### `Node` (enum)
- 役割: ネットリストを構成するノード。計算の単位。
- バリアント:
  - `Const { id: NodeId, value: u64, width: u64 }` — 定数
    - `id` — ノードID
    - `value` — 定数の値
    - `width` — ビット幅
  - `ReadSignal { id: NodeId, signal_id: usize, signal_name: String, width: u64 }` — 信号読み出し
    - `signal_id` — 読み出す信号のID
    - `signal_name` — デバッグ用の信号名
    - `width` — ビット幅
  - `BinOp { id: NodeId, op: BinOp, lhs: NodeId, rhs: NodeId, width: u64 }` — 二項演算
    - `op` — 演算子
    - `lhs` — 左辺のノードID
    - `rhs` — 右辺のノードID
    - `width` — 結果のビット幅
  - `Drive { id: NodeId, signal_id: usize, signal_name: String, source: NodeId, kind: DriveKind }` — 信号駆動
    - `signal_id` — 駆動する信号のID
    - `signal_name` — デバッグ用の信号名
    - `source` — 駆動値のソースノードID
    - `kind` — 駆動の種類（Combinational/Sequential）

#### `DriveKind` (enum)
- 役割: 信号駆動の種類。
- バリアント:
  - `Combinational` — 組み合わせ（`=`、即時反映）
  - `Sequential` — 順序（`<=`、サイクル終了時に一斉反映）

- `Display` 実装: `Combinational` → `"blocking"`, `Sequential` → `"non-blocking"`

#### `Netlist`
- 役割: 生成されたネットリスト全体。
- フィールド:
  - `signals: Vec<NetlistSignal>` — 全信号のリスト
  - `nodes: Vec<Node>` — 全ノードのリスト（DAG の頂点集合）

#### `NetlistSignal`
- 役割: ネットリスト上の信号情報。
- フィールド:
  - `id: usize` — 信号ID
  - `name: String` — 信号名
  - `width: u64` — ビット幅
  - `driver_node: Option<NodeId>` — この信号を駆動する Drive ノードのID（未駆動 = None）
  - `driver_kind: Option<DriveKind>` — 駆動の種類（未駆動 = None）

#### `NetlistBuilder`
- 役割: 内部ビルダー。ノードを生成・追加しながら Netlist を構築する。
- フィールド:
  - `nodes: Vec<Node>` — 構築中のノードリスト
  - `next_id: NodeId` — 次に割り当てるノードID

- メソッド:
  - `new() -> Self` — 空のビルダーを生成
  - `alloc_id(&mut self) -> NodeId` — 新しいノードIDを割り当てる
  - `add_node(&mut self, node: Node) -> NodeId` — ノードを追加し、そのIDを返す
  - `make_const(&mut self, value, width) -> NodeId` — 定数ノードを生成
  - `make_read_signal(&mut self, signal_id, name, width) -> NodeId` — 信号読み出しノードを生成
  - `make_binop(&mut self, op, lhs, rhs, width) -> NodeId` — 二項演算ノードを生成
  - `make_drive(&mut self, signal_id, name, source, kind) -> NodeId` — 駆動ノードを生成
  - `build_expr(&mut self, expr, signals) -> NodeId` — 解決済み式からノードを構築
  - `node_width(&self, node_id) -> u64` — ノードのビット幅を取得

#### `build_netlist(elab: &Elaborated) -> Netlist`
- 概要: エラボレーション結果からネットリストを生成する。
- 処理:
  1. `Elaborated.signals` から `NetlistSignal` のリストを作成（driver情報は初期化時点では None）
  2. 各 `ResolvedStmt` について:
     - `build_expr()` で右辺の式ノードを構築
     - `make_drive()` で駆動ノードを生成
     - 対応する信号の `driver_node`/`driver_kind` を更新
  3. `Netlist { signals, nodes }` を返す

#### `format_netlist(nl: &Netlist) -> String`
- 概要: ネットリストを人間が読めるテキスト形式に整形する。
- 出力例:
  ```
  ===== Netlist =====

  --- Signals ---
    a[0:0] : bit  (id=0)
                 driven by N3 (blocking)
    b[0:0] : bit  (id=1)
                 driven by N5 (non-blocking)

  --- Nodes ---
    N  0: Read(b)  (1 bit)
    N  1: Const(1)  (1 bit)
    N  2: Xor  (1 bit)  = N0 ^ N1
    N  3: Drive(a)  (blocking)  <= N2
    N  4: Read(a)  (1 bit)
    N  5: Drive(b)  (non-blocking)  <= N4
  ```

---

### `simulator` モジュール

#### `CycleSnapshot`
- 役割: 1サイクル分のシミュレーション結果。
- フィールド:
  - `cycle: u64` — サイクル番号（0始まり）
  - `values: Vec<u64>` — 各信号の値（信号ID順、インデックス = 信号ID）

#### `Simulator`
- 役割: ネットリストを評価して波形を生成する。
- フィールド:
  - `signal_values: Vec<u64>` — 現在の各信号の値（信号ID順）
  - `cycle: u64` — 経過サイクル数

##### `Simulator::new(signal_count: usize) -> Self`
- 概要: 全信号を0で初期化したシミュレーターを生成する。
- 引数: `signal_count` — 信号の数

##### `Simulator::set_signal(&mut self, id: usize, value: u64)`
- 概要: 特定の信号に初期値を設定する（step() 実行前に呼ぶ）。

##### `Simulator::signal_values(&self) -> &[u64]`
- 概要: 現在の全信号値をスライスで返す。

##### `Simulator::cycle(&self) -> u64`
- 概要: 現在のサイクル数を返す。

##### `Simulator::step(&mut self, nodes: &[Node]) -> CycleSnapshot`
- 概要: 1サイクル分シミュレーションを進め、結果のスナップショットを返す。
- 処理:
  1. スナップショット取得: サイクル開始時の全信号値をクローンする（ノンブロッキング参照用）
  2. Phase 1 — 組み合わせ評価（Δ-サイクル、最大1000回）:
     - 全コンビネーション Drive ノードを評価し、信号値を即時更新
     - 値が収束するまで（変更がなくなるまで）ループ
     - 1000回の反復で収束しなければ組合せループと判定してパニック
  3. Phase 2 — 順序評価:
     - 全シーケンシャル Drive ノードを評価（参照する値は Phase 1 開始前のスナップショット）
     - 評価結果を `next` 配列に格納
     - `next` → `signal_values` に一斉コミット
  4. サイクルカウンタを進め、`CycleSnapshot` を返す

##### `Simulator::run(&mut self, nodes: &[Node], cycles: u64) -> Vec<CycleSnapshot>`
- 概要: Nサイクル連続で実行し、全スナップショットを返す。
- 引数: `cycles` — 実行するサイクル数
- 返り値: `Vec<CycleSnapshot>` — サイクル0〜N-1 のスナップショット

#### `eval_node(node_id: NodeId, nodes: &[Node], signal_values: &[u64]) -> u64`
- 概要: ノードID を指定して、そのノードの出力値を再帰的に計算する。
- 処理:
  - `Const` → 保持している定数値を返す
  - `ReadSignal` → `signal_values` から該当信号の値を返す
  - `BinOp(Xor)` → 左右の子ノードを再帰評価して XOR をとる
  - `Drive` → ソースノードを再帰評価して返す（値をそのまま中継）

#### `format_waveform(snapshots: &[CycleSnapshot], signals: &[NetlistSignal]) -> String`
- 概要: シミュレーション結果を見やすいテキスト波形表に整形する。
- 出力例:
  ```
  cycle  a  b
  -----------
      0  1  0
      1  1  1
      2  0  1
      3  0  0
  ```

---

## CLI 使用方法

```bash
# サンプルコードを実行（ネットリスト表示まで）
cargo run

# ファイルを指定
cargo run -- path/to/file.tc

# シミュレーションまで実行（Nサイクル）
cargo run -- path/to/file.tc --cycles 10
cargo run -- --cycles 6   # サンプルコードを6サイクル
```

### オプション

| 引数 | 説明 |
|---|---|
| `--cycles N`, `-c N` | シミュレーションを N サイクル実行する |

---

## シミュレーションモデル

### 1サイクルの動作

```
サイクル開始 (signal_values = 現在値)
  │
  ├─ スナップショット: snapshot = signal_values のコピー
  │
  ├─ Phase 1: 組み合わせ評価（Δ-サイクル）
  │   ループ:
  │     全 Combinational Drive を評価
  │     signal_values を即時更新
  │     変更がなければ終了
  │
  ├─ Phase 2: 順序評価
  │     全 Sequential Drive を評価（参照値 = snapshot）
  │     next 配列に結果を貯める
  │     signal_values = next（一斉コミット）
  │
  └─ cycle++、結果出力
```

### ノンブロッキング代入の動作

`b <= a` の例:

| Cycle | 開始時 a | Phase1 (comb) | Phase2 (seq: b) | 終了時 b |
|---|---|---|---|---|
| 0 | 0 | 変化なし | snapshot[a]=0 を評価 | 0 |
| 1 | 0 | a が comb で 1 に更新 | snapshot[a]=0 を評価 | 0 |
| 2 | 1 | 変化なし | snapshot[a]=1 を評価 | 1 |

→ b は a を1サイクル遅れで追従する。

---

## 拡張ポイント

現在のアーキテクチャで新しい機能を追加するときの変更箇所:

| 追加したい機能 | 変更するファイル |
|---|---|
| 新しい演算子（&, \|, +, -） | `ast.rs` (BinOp), `lexer.rs` (Token), `parser.rs` (parse_expr), `netlist.rs` (format), `simulator.rs` (eval_node) |
| ビットベクタリテラル（`7'b000_0001`） | `lexer.rs` (read_number 拡張), `ast.rs` (Expr 拡張) |
| if/case 文 | `ast.rs` (Stmt 拡張), `parser.rs` (parse_stmt), `netlist.rs` (Node 拡張), `simulator.rs` |
| モジュール・ポート | `ast.rs` (Module 追加), `parser.rs`, `elaboration.rs` (階層解決) |
| VCD ダンプ | `simulator.rs` (format_waveform の代わりに VCD 出力) |