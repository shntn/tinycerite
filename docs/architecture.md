__Tiny Cerilte — アーキテクチャドキュメント__


tinycerilte は最小の HDL シミュレータ。入出力は以下。

```
Source (.tc)
  ↓ Parser (pest) — 字句解析 + 構文解析
AST (Program)
  ↓ Elaboration — モジュール定義解決・シンボル解決・型解決 + 静的チェック
Elaborated IR（モジュール階層を保持）
  ↓ Netlist Builder — モジュール階層をフラットなDAGへ展開
Netlist (信号DAG、階層情報は名前空間プレフィックスのみ)
  ↓ Simulator — Δ-サイクル評価（テストベンチの`initial`があればその手続きに従って駆動）
波形出力
```

---

# 言語仕様 (Language)

## 文法 (EBNF)

```
program     = (module_def | testbench_def)+

# モジュール定義
module_def  = "module" ident "{" port_block (decl | stmt)* "}"
port_block  = "port" "{" port_decl* "}"
port_decl   = ident ":" ("input" | "output") signal_type ";"
signal_type = "clock" | ("bit" ("<" number ">")?)

# テストベンチ定義（プログラム中に高々1つ。トップレベルの信号空間はここでのみ構築される）
testbench_def = "testbench" ident "{" (decl | inst_decl | stmt)* initial_block? "}"
initial_block = "initial" "{" (proc_assign | proc_step)* "}"
proc_assign   = ident "=" ternary_expr ";"
proc_step     = "step" ";"

decl        = "var" ident ":" signal_type ";"
# モジュールインスタンス化（testbench_def内のみ。モジュール本体はネスト不可）
inst_decl   = "var" ident "=" ident "(" (named_arg ("," named_arg)*)? ")" ";"
named_arg   = ident ":" ternary_expr

stmt        = ident ("=" | "<=") ternary_expr ";"

# 三項演算子（右結合、演算子の中で最も優先度が低い）
ternary_expr      = expression ("?" ternary_expr ":" ternary_expr)?

# 演算子優先順位チェーン（上ほど優先度が低い）
expression        = expression1 ("||" expression1)*
expression1       = expression2 ("&&" expression2)*
expression2       = expression3 ("|" expression3)*
expression3       = expression4 ("^" expression4)*
expression4       = expression5 ("&" expression5)*
expression5       = expression6 (("==" | "!=") expression6)*
expression6       = expression7 (("<=" | "<" | ">=" | ">") expression7)*
expression7       = expression8 (("<<<" | "<<" | ">>>" | ">>") expression8)*
expression8       = expression9 (("+" | "-") expression9)*
expression9       = expression_unary (("*" | "/" | "%") expression_unary)*
expression_unary  = ("!" | "~")* expression_factor
expression_factor = field_access | ident | bitvec_literal | number | "(" ternary_expr ")"
field_access      = ident "." ident   # instance.output_port

ident          = [a-zA-Z_][a-zA-Z0-9_]*
number         = [0-9]+
bitvec_literal = number "'" ("b" | "o" | "d" | "h") [0-9a-zA-Z]+
```

## セマンティクス

- `var x: bit` — 1ビットの信号 x を宣言（初期値 0）
- `var x: bit<N>` — Nビットの信号 x を宣言
- `var x: clock` — クロック型の信号 x を宣言（常に1ビット、`<N>`は書けない）。**テストベンチ内でのみ**許可される（モジュール本体で書くとエラー）。詳細は「クロック」節を参照
- `N'b...` `N'o...` `N'd...` `N'h...` — ビットベクタリテラル。幅 `N` と基数（`b`=2進、`o`=8進、`d`=10進、`h`=16進、大小文字どちらの16進桁も可）を明示する（例: `4'b1010`、`8'hFF`）。桁が宣言した幅に収まらない場合（例: `4'b11111`）はエラーにせず、代入と同様に幅へ切り詰める。基数に合わない桁（例: `2'b19`）はパースエラー
- `a = expr;` — 組み合わせ代入（即時反映）
- `a <= expr;` — 順序代入（サイクル開始時の値で評価、サイクル終了時に一斉反映）
- `cond ? then_branch : else_branch` — 三項演算子（条件式）。すべての演算子の中で最も優先度が低く、右結合（`a ? b : c ? d : e` は `a ? b : (c ? d : e)`）。`cond` が0以外なら `then_branch`、0なら `else_branch` を選択する。ハードウェア的にはマルチプレクサなので、選ばれなかった分岐も含めて両方が常に評価される（0除算などの副作用があっても選択に関わらず発生する）。結果幅は `then_branch`/`else_branch` の大きい方（`cond` は選択にのみ使われ幅には影響しない）
- `module name { port { ... } (var宣言 | 代入文)* }` — モジュール定義。`port { }` ブロックに入出力ポート（`input`/`output`）を宣言し、続けて内部信号の宣言・代入文を書く。`input` ポートへの内部代入はエラー。モジュール定義は宣言された時点で（インスタンス化の有無によらず）1回だけ解決・検証される
  - モジュール内で順序代入（reg相当）を1つでも使う場合、そのモジュールに`clock`型の入力ポートが必須（無いとエラー）。`clock`型の入力ポートは高々1つ（2つ以上はエラー）。`output`に`clock`型は使えない（エラー）。詳細は「クロック」節を参照
- `var 名前 = モジュール名(ポート名: 式, ...);` — モジュールインスタンス化。`input` ポートだけを名前付き引数として接続する（構造体リテラルではなく関数呼び出し風の構文）。全`input`ポート分の接続が必須で、過不足・`output`ポートの指定はエラー。`clock`型ポートへの接続には`clock`型の信号のみ許可される（逆に`clock`型の信号を`bit`型ポートに繋ぐのもエラー）
- `インスタンス名.出力ポート名` — インスタンスの出力を式の中で読み出す（例: `z = u1.sum + 1;`）。`input`ポートを外部から読み出そうとするとエラー
- `testbench name { (decl | inst_decl | stmt)* initial_block? }` — テストベンチ定義。プログラム中に高々1つ（2つ以上あるとエラー）。トップレベルの信号空間はここでのみ構築される（`decl`/`inst_decl`/`stmt`以外の場所に信号を宣言する手段は無い）。クロック信号もここで普通の代入文として作る（例: `clk <= !clk;`で毎サイクルトグル、`clk <= counter & 1;`で分周。1回の`Simulator::step`が最小の時間刻みなので、クロックの1周期は自然に2ステップになる。`clock`型として宣言するかどうかは任意だが、モジュールの`clock`型ポートに接続するには`clock`型として宣言されている必要がある）
- `initial { (proc_assign | proc_step)* }` — テストベンチ内の手続き部分（省略可）。並行部分とは異なる意味論を持つ: 上から順に、1文ずつ実行される
  - `proc_assign`（`target = expr;`）— その瞬間に一度だけ`target`に値を設定する（継続的な駆動ではない。`Simulator::set_signal`相当）
  - `proc_step`（`step;`）— シミュレーションを明示的に1サイクル進める（`Simulator::step`相当）
- 演算子（優先度が高い順）:
  1. `!` `~` — 前置単項演算子。論理否定（`!`、結果は1ビットの真偽値）とビット反転（`~`、結果はオペランドと同じ幅）。連続して書ける（例: `!!a`、右結合で内側から適用）
  2. `*` `/` `%` — 乗算・除算・剰余（0除算は0を返す）
  3. `+` `-` — 加算・減算（u64のラップアラウンドで近似）
  4. `<<` `>>` `<<<` `>>>` — 論理/算術シフト（現状は同じ動作。シフト量が64以上は0を返す）
  5. `<` `<=` `>` `>=` — 大小比較（結果は1ビットの真偽値）
  6. `==` `!=` — 等値比較（結果は1ビットの真偽値）
  7. `&` — ビット単位 AND
  8. `^` — ビット単位 XOR
  9. `|` — ビット単位 OR
  10. `&&` — 論理 AND（結果は1ビットの真偽値）
  11. `||` — 論理 OR（結果は1ビットの真偽値）

## サンプル

```
testbench tb {
    var a: bit;
    var b: bit;

    a = b ^ 1;    // 組み合わせ: a は b の反転
    b <= a;       // 順序: b は a を1サイクル遅れで追従
}
```

この回路はトグルフリップフロップ（TFF）として動作し、a は2サイクルごとに 1→0 を繰り返す。

## モジュールの例

```
module adder {
    port {
        a: input bit<8>;
        b: input bit<8>;
        sum: output bit<8>;
    }

    sum = a + b;
}

testbench tb {
    var x: bit<8>;
    var y: bit<8>;
    var z: bit<8>;

    var u1 = adder(a: x, b: y);   // inputポートだけを名前付き引数で接続
    z = u1.sum;                    // outputポートはインスタンスから読み出す
}
```

現状の制限:

- モジュールが別のモジュールをインスタンス化すること（ネスト）は未対応（文法上、`inst_decl` は `testbench_def` の中にのみ許可されている）
- インスタンス境界をまたぐ組合せループ（あるインスタンスの出力を同じインスタンスの入力に戻すような配線）はエラボレーション時点では検出できない。詳細は後述の `elaboration` モジュール節を参照

## クロックの例

```
module adder {
    port { clk: input clock; a: input bit<8>; b: input bit<8>; sum: output bit<8>; }
    sum <= a + b;   // 順序代入(reg) → clock型入力ポートが無いとエラボレーションエラー
}

testbench tb {
    var clk: clock;      // clock型のvar宣言はテストベンチ内でのみ許可される
    clk <= !clk;

    var x: bit<8>;
    var y: bit<8>;
    var u1 = adder(clk: clk, a: x, b: y);   // clock型ポートにはclock型の信号のみ接続できる
}
```

- `clk: input clock;` — ポートを`clock`型として宣言する。`output`には使えない
- `var clk: clock;` — クロック信号の宣言。テストベンチ内でのみ許可される。値の生成方法自体は普通の代入文のまま（`clk <= !clk;`で毎ステップ反転、`clk <= counter & 1;`で分周）
- モジュール内に順序代入が1つでもあれば`clock`型入力ポートが必須、`clock`型入力ポートは高々1つ
- インスタンス化時の接続は`clock`型どうし・`bit`型どうしでしか繋げない（型不一致はエラー）
- regの`clock`はそのモジュールの`clock`入力ポートに紐付く。モジュールの外（トップレベル/テストベンチ直下）の順序代入はモジュールに属さないため対象外
- regは実際にクロックの立ち上がりエッジでのみ更新される（`Simulator::step`が`SignalKind::Reg.clock`を見てエッジ検出する）。クロック紐付けが無いreg（モジュールの外のreg）は今まで通り毎ステップ更新される。現状トリガーの向きはposedge固定（暫定処置）。詳細は後述の`simulator`モジュール節を参照

## テストベンチの例

```
module adder {
    port { clk: input clock; a: input bit<8>; b: input bit<8>; sum: output bit<8>; }
    sum <= a + b;
}

testbench tb {
    var counter: bit<8>;
    counter <= counter + 1;

    var clk: clock;
    clk <= counter & 1;

    var x: bit<8>;
    var y: bit<8>;
    var z: bit<8>;
    var u1 = adder(clk: clk, a: x, b: y);

    initial {
        x = 3;
        y = 4;
        step;
        step;
        z = u1.sum;
    }
}
```

`initial`がある場合、CLIの`--cycles`は無視され、`initial`の手続き（`step;`の回数）に従って実行される。`initial`が無い（または`testbench`自体が無い）場合は今まで通り`--cycles N`でNサイクル実行する。

現状の制限:

- `assert`のような値検証構文は無い（波形出力を見て手動で確認する）
- `testbench`はプログラム中に高々1つ（2つ以上あるとエラボレーションエラー）

---

# モジュール一覧

| モジュール     | ファイル             | 役割                                     |
|----------------|----------------------|------------------------------------------|
| `grammar.pest` | `src/grammar.pest`   | PEG 文法定義（pest が読み込む）          |
| `ast`          | `src/ast.rs`         | AST 型定義                               |
| `parser`       | `src/parser.rs`      | 構文解析（Source → AST, pest ラッパー） |
| `elaboration`  | `src/elaboration.rs` | シンボル解決・型解決                     |
| `netlist`      | `src/netlist.rs`     | ネットリスト生成（DAG構築）              |
| `simulator`    | `src/simulator.rs`   | シミュレーション実行                     |
| `main`         | `src/main.rs`        | CLI エントリポイント                     |

---

# モジュール詳細

各モジュールを「型（データ）」と「関数（処理）」に分けて説明する。

## `ast` モジュール

### 型

`Program` :

- 役割: パース結果のトップレベル。0個以上のモジュール定義・テストベンチ定義を持つ。
- フィールド:
  - `modules: Vec<ModuleDef>` — プログラム中のモジュール定義のリスト
  - `testbenches: Vec<TestbenchDef>` — プログラム中のテストベンチ定義のリスト（文法上は複数書けるが、高々1つという制約はエラボレーションで検証する）

`Decl` :

- 役割: `var name: bit<N>;` / `var name: clock;` による変数宣言。
- フィールド:
  - `name: String` — 変数名
  - `sig_type: SignalType` — 信号の型

`SignalType` (enum) :

- 役割: 信号の型（`bit`/`bit<N>`/`clock`）。`Decl`/`PortDecl`で共用する。
- バリアント:
  - `Bit(Option<u64>)` — `None` = `bit`（1ビット）、`Some(n)` = `bit<n>`（Nビット）
  - `Clock` — クロック型。常に1ビット扱いで`<N>`は書けない
- メソッド:
  - `width(&self) -> u64` — ビット幅を返す（`Bit(None)`/`Clock`は1、`Bit(Some(n))`は`n`）
  - `is_clock(&self) -> bool` — `Clock`かどうか

`Direction` (enum) :

- 役割: ポートの向き。
- バリアント: `Input` `Output`

`PortDecl` :

- 役割: `name: input/output bit<N>;` / `name: input/output clock;` によるポート宣言（モジュール定義の`port { }`ブロック内）。
- フィールド:
  - `name: String` — ポート名
  - `direction: Direction` — 向き
  - `sig_type: SignalType` — 信号の型（`Decl`と同じ規則。`clock`型を`output`に使うと後段の`elaboration`でエラーになる）

`ModuleDef` :

- 役割: `module name { port { ... } ... }` によるモジュール定義。
- フィールド:
  - `name: String` — モジュール名
  - `ports: Vec<PortDecl>` — ポート宣言のリスト
  - `decls: Vec<Decl>` — モジュール内部の変数宣言のリスト
  - `stmts: Vec<Stmt>` — モジュール内部の代入文のリスト

`InstDecl` :

- 役割: `var name = module_name(port: expr, ...);` によるモジュールインスタンス化宣言。
- フィールド:
  - `instance_name: String` — インスタンス名
  - `module_name: String` — インスタンス化するモジュール名
  - `args: Vec<(String, Expr)>` — 名前付き引数（`inputポート名, 接続式`）のリスト

`TestbenchDef` :

- 役割: `testbench name { ... }` によるテストベンチ定義。
- フィールド:
  - `name: String` — テストベンチ名
  - `decls: Vec<Decl>` — 並行部分の変数宣言のリスト
  - `instances: Vec<InstDecl>` — 並行部分のモジュールインスタンス化のリスト
  - `stmts: Vec<Stmt>` — 並行部分の代入文のリスト（トップレベルの信号空間はここでのみ構築される）
  - `initial: Vec<ProcStmt>` — 手続き部分（`initial { }`）の文のリスト。`decls`/`instances`/`stmts`とは異なる意味論を持つ

`ProcStmt` (enum) :

- 役割: `initial { }` 内の手続き文。
- バリアント:
  - `Assign { target: String, expr: Expr }` — `target = expr;`。その瞬間に一度だけ値を設定する（継続的な駆動ではない）
  - `Step` — `step;`。シミュレーションを1サイクル進める

`Stmt` (enum) :

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

`Expr` (enum) :

- 役割: 式。右辺の計算を表す木構造。
- バリアント:
  - `Ident(String)` — 変数参照（例: `a`）
  - `Number(u64)` — 10進数リテラル（例: `1`、`42`）。幅は代入先や周囲の式から推測される
  - `BitVecLiteral { width: u64, value: u64 }` — ビットベクタリテラル（例: `4'b1010`、`8'hFF`）。`Number`と異なり幅を明示する
    - `width` — 明示された幅
    - `value` — パース済みの値（基数変換後。宣言幅に収まらない場合、幅への切り詰めはネットリスト構築時に行う）
  - `FieldAccess { instance: String, field: String }` — モジュールインスタンスの出力ポート参照（例: `u1.sum`）
    - `instance` — インスタンス名
    - `field` — 参照するポート名
  - `BinOp { op: BinOp, lhs: Box<Expr>, rhs: Box<Expr> }` — 二項演算
    - `op` — 演算子の種類
    - `lhs` — 左辺の式
    - `rhs` — 右辺の式
  - `UnaryOp { op: UnOp, expr: Box<Expr> }` — 前置単項演算
    - `op` — 演算子の種類
    - `expr` — オペランドの式
  - `Ternary { cond: Box<Expr>, then_branch: Box<Expr>, else_branch: Box<Expr> }` — 三項演算（条件式）
    - `cond` — 条件式
    - `then_branch` — 条件が真の場合の式
    - `else_branch` — 条件が偽の場合の式

`BinOp` (enum) :

- 役割: 二項演算子の種類。
- バリアント: `Or`(`||`) `And`(`&&`) `BitOr`(`|`) `Xor`(`^`) `BitAnd`(`&`) `Eq`(`==`) `Neq`(`!=`) `Lt`(`<`) `Le`(`<=`) `Gt`(`>`) `Ge`(`>=`) `Shl`(`<<`) `Shr`(`>>`) `AShl`(`<<<`) `AShr`(`>>>`) `Add`(`+`) `Sub`(`-`) `Mul`(`*`) `Div`(`/`) `Mod`(`%`)

- `Display` 実装: 各バリアントを対応する演算子記号に変換する（上記カッコ内の記号）

`UnOp` (enum) :

- 役割: 前置単項演算子の種類。
- バリアント: `Not`(`!`、論理否定) `BitNot`(`~`、ビット反転)

- `Display` 実装: 各バリアントを対応する演算子記号に変換する（上記カッコ内の記号）

---

## `parser` モジュール

### 型

`ParseError` :

- 役割: 構文解析エラー（pest のエラーをラップ）。
- フィールド:
  - `message: String` — エラーメッセージ

- `Display` 実装: `"パースエラー: <message>"`

`CeriteParser` :

- 役割: pest の derive マクロで `grammar.pest` から自動生成されたパーサー。
- `pest_derive::Parser` を継承し、`pest::Parser` trait の `parse()` メソッドを実装する。

`Parser` :

- 役割: pest パーサーのラッパー。公開 API を提供する。
- フィールド: なし（ユニット構造体）

### 関数

`Parser::parse_program(input: &str) -> Result<Program>` :

- 概要: 入力文字列をパースし、`Program` を返す。
- 処理:
  1. `CeriteParser::parse(Rule::program, input)` を呼び、pest の `Pairs` を得る
  2. `Pairs` を走査し、`Rule::program` の子ペアから `Rule::module_def` → `parse_module_def()`、`Rule::testbench_def` → `parse_testbench_def()` に振り分けて `Program` を構築

`parse_module_def(pair: Pair<Rule>) -> Result<ModuleDef>` :

- 概要: `module_def`（`"module" ~ ident ~ "{" ~ port_block ~ (decl | stmt)* ~ "}"`）から `ModuleDef` を構築。
- 処理: 子ペアから `Rule::ident` → モジュール名（`is_keyword`でキーワード検査）、`Rule::port_block` → `parse_port_block()`、`Rule::decl`/`Rule::stmt` → それぞれ `parse_decl()`/`parse_stmt()` に振り分け。

`parse_port_block(pair: Pair<Rule>) -> Result<Vec<PortDecl>>` :

- 概要: `port_block`（`"port" ~ "{" ~ port_decl* ~ "}"`）から `PortDecl` のリストを構築。
- 処理: 子ペアの `Rule::port_decl` をそれぞれ `parse_port_decl()` で変換。

`parse_port_decl(pair: Pair<Rule>) -> Result<PortDecl>` :

- 概要: `port_decl`（`ident ~ ":" ~ direction ~ signal_type ~ ";"`）から `PortDecl` を構築。
- 処理: 子ペアから `Rule::ident` → ポート名（`is_keyword`でキーワード検査）、`Rule::direction` → `input`→`Direction::Input`、`output`→`Direction::Output`、`Rule::signal_type` → `parse_signal_type()`で`SignalType`を抽出。

`parse_signal_type(pair: Pair<Rule>) -> Result<SignalType>` :

- 概要: `signal_type`（`"clock" | ("bit" ~ ("<" ~ number ~ ">")?)`）から `SignalType` を構築。`port_decl`/`decl`の両方から共通で呼ばれる。
- 処理: マッチしたテキスト（`pair.as_str()`）が`"clock"`で始まれば`SignalType::Clock`を返す。それ以外は`bit`側なので、子ペアの`Rule::number`（あれば）を幅として`SignalType::Bit(width)`を返す。

`parse_testbench_def(pair: Pair<Rule>) -> Result<TestbenchDef>` :

- 概要: `testbench_def`（`"testbench" ~ ident ~ "{" ~ (decl | inst_decl | stmt)* ~ initial_block? ~ "}"`）から `TestbenchDef` を構築。
- 処理: 子ペアから `Rule::ident` → テストベンチ名（`is_keyword`でキーワード検査）、`Rule::decl`/`Rule::inst_decl`/`Rule::stmt` → それぞれ `parse_decl()`/`parse_inst_decl()`/`parse_stmt()` に振り分け、`Rule::initial_block` → `parse_initial_block()`。

`parse_initial_block(pair: Pair<Rule>) -> Result<Vec<ProcStmt>>` :

- 概要: `initial_block`（`"initial" ~ "{" ~ (proc_assign | proc_step)* ~ "}"`）から `ProcStmt` のリストを構築。
- 処理: `Rule::proc_assign` → `parse_proc_assign()`、`Rule::proc_step` → `ProcStmt::Step` を直接追加。

`parse_proc_assign(pair: Pair<Rule>) -> Result<ProcStmt>` :

- 概要: `proc_assign`（`ident ~ "=" ~ ternary_expr ~ ";"`）から `ProcStmt::Assign { target, expr }` を構築。
- 処理: 子ペアから `Rule::ident` → 対象の変数名（`is_keyword`でキーワード検査）、`Rule::ternary_expr` → `parse_ternary_expr` で右辺を解決。

`parse_inst_decl(pair: Pair<Rule>) -> Result<InstDecl>` :

- 概要: `inst_decl`（`"var" ~ ident ~ "=" ~ ident ~ "(" ~ (named_arg ~ ("," ~ named_arg)*)? ~ ")" ~ ";"`）から `InstDecl` を構築。
- 処理: 子ペアの `Rule::ident` を出現順に走査し、1つ目をインスタンス名（`is_keyword`でキーワード検査）、2つ目をモジュール名として扱う。`Rule::named_arg` はそれぞれ `parse_named_arg()` で変換して `args` に集める。

`parse_named_arg(pair: Pair<Rule>) -> Result<(String, Expr)>` :

- 概要: `named_arg`（`ident ~ ":" ~ ternary_expr`）から `(ポート名, 接続式)` のタプルを構築。

`parse_decl(pair: Pair<Rule>) -> Result<Decl>` :

- 概要: `decl`（`"var" ~ ident ~ ":" ~ signal_type ~ ";"`）ルールのペアから `Decl` を構築。
- 処理: 子ペアから `Rule::ident` → 変数名、`Rule::signal_type` → `parse_signal_type()`で`SignalType`を抽出。変数名は `is_keyword` でキーワードでないか検査し、キーワードならエラー。

`parse_stmt(pair: Pair<Rule>) -> Result<Stmt>` :

- 概要: `stmt` ルールのペアから `Stmt` を構築。
- 処理: 子ペアから `Rule::ident` → 代入先、`Rule::assign` or `Rule::nonblock` → 代入種別、`Rule::ternary_expr` → 右辺を抽出。代入先の変数名は `is_keyword` でキーワードでないか検査し、キーワードならエラー。

`parse_ternary_expr(pair: Pair<Rule>) -> Result<Expr>` :

- 概要: `ternary_expr`（`expression ~ ("?" ~ ternary_expr ~ ":" ~ ternary_expr)?`）を解決する。文法上、式の最上位（`stmt` の右辺・括弧の中身）はすべてこの関数から入る。
- 処理: 条件部（`Rule::expression`）を `parse_expression` で解決。`?`/`:` に続く then/else が無ければ条件部の式をそのまま返す（三項演算子は常に使われるわけではない）。ある場合は then/else をそれぞれ再帰的に `parse_ternary_expr` で解決して `Expr::Ternary` を組み立てる。then/elseの解決を再帰にすることで、`a ? b : c ? d : e` が `a ? b : (c ? d : e)`（右結合）になる。

`parse_left_assoc(pair, parse_operand, op_from_str) -> Result<Expr>` :

- 概要: `operand (op operand)*` の形をした優先順位チェーンの1段を、左結合の `Expr::BinOp` 木に組み立てる共通ヘルパー。9段ある `expression`〜`expression9` はすべてこの関数を呼ぶだけの薄いラッパーになっている。
- 引数:
  - `parse_operand` — 1つ下の優先順位のペアを解決する関数（例: `parse_expression2`）
  - `op_from_str` — 演算子ルール（`or_op`/`eq_op`など）の一致文字列（`as_str()`）から `BinOp` バリアントへ変換する関数
- 処理: 最初のオペランドを解決し、以降 `(演算子, オペランド)` の組が続く限り左結合で `Expr::BinOp` に畳み込む。

`parse_expression`〜`parse_expression9(pair: Pair<Rule>) -> Result<Expr>` :

- 概要: 文法の優先順位チェーン（`expression`〜`expression9`）に1対1対応する9つの関数。すべて `parse_left_assoc` に、1つ下の段の解析関数と演算子変換関数を渡すだけ。
- 対応表（上ほど優先度が低い）:

  | 関数 | 演算子ルール | 演算子 → BinOp |
  |---|---|---|
  | `parse_expression` | `or_op` | `\|\|` → `Or` |
  | `parse_expression1` | `and_op` | `&&` → `And` |
  | `parse_expression2` | `bitor_op` | `\|` → `BitOr` |
  | `parse_expression3` | `xor_op` | `^` → `Xor` |
  | `parse_expression4` | `bitand_op` | `&` → `BitAnd` |
  | `parse_expression5` | `eq_op` | `==`→`Eq`, `!=`→`Neq` |
  | `parse_expression6` | `rel_op` | `<=`→`Le`, `<`→`Lt`, `>=`→`Ge`, `>`→`Gt` |
  | `parse_expression7` | `shift_op` | `<<<`→`AShl`, `<<`→`Shl`, `>>>`→`AShr`, `>>`→`Shr` |
  | `parse_expression8` | `add_op` | `+`→`Add`, `-`→`Sub` |
  | `parse_expression9` | `mul_op` | `*`→`Mul`, `/`→`Div`, `%`→`Mod` |

`parse_expression_unary(pair: Pair<Rule>) -> Result<Expr>` :

- 概要: `expression_unary`（`unary_op* ~ expression_factor`）を右結合の `Expr::UnaryOp` 木に組み立てる。優先順位チェーンの最下段（`parse_expression9`）が呼ぶ、`expression_factor` の直前の1段。
- 処理: 子ペアを走査して `Rule::unary_op`（`!`→`Not`、`~`→`BitNot`）を出現順に集め、`Rule::expression_factor` を `parse_expression_factor` で解決。集めた演算子を逆順（オペランドに近い方から）に適用し、`!~a` が `Not(BitNot(a))` になるようネストする。

`parse_expression_factor(pair: Pair<Rule>) -> Result<Expr>` :

- 概要: `expression_factor` ルールのペアから `Expr::FieldAccess`・`Expr::Ident`・`Expr::Number`・`Expr::BitVecLiteral`、または括弧で囲まれた `Expr`（`ternary_expr` を再帰的に解決）を構築する。
- 処理: 子ペアのルール種別を見て、`Rule::ident` → `Ident(name)`（`is_keyword` でキーワード検査）、`Rule::number` → `Number(value)`、`Rule::bitvec_literal` → `parse_bitvec_literal` を呼び出し、`Rule::field_access` → `parse_field_access` を呼び出し、`Rule::ternary_expr` → `parse_ternary_expr` を再帰呼び出し（括弧の中身。括弧内にも三項演算子を書けるようにするため `expression` ではなく `ternary_expr` を再帰する）。

`parse_bitvec_literal(pair: Pair<Rule>) -> Result<Expr>` :

- 概要: `bitvec_literal`（`number ~ "'" ~ radix ~ literal_digits`）を `Expr::BitVecLiteral` に変換する。
- 処理: 子ペアから幅（`Rule::number`）、基数文字（`Rule::radix`: `b`/`o`/`d`/`h`）、桁の文字列（`Rule::literal_digits`）を取り出し、基数を2/8/10/16に対応させて `u64::from_str_radix` で値へ変換する。基数に合わない桁（例: `2進数`基数に対する`9`）は `from_str_radix` がエラーを返すので、そのまま `ParseError` として伝播する。宣言幅への切り詰めはここでは行わず（`ResolvedExpr`・`Node::Const` まではそのまま値を保持し）、ネットリスト構築時（`NetlistBuilder::build_expr`）にまとめて行う。

`parse_field_access(pair: Pair<Rule>) -> Result<Expr>` :

- 概要: `field_access`（`ident ~ "." ~ ident`）を `Expr::FieldAccess { instance, field }` に変換する。
- 処理: 子ペアの2つの `Rule::ident` を順にインスタンス名・フィールド名として取り出す（どちらも `is_keyword` でキーワード検査）。この時点ではインスタンスが実在するか・フィールドがそのモジュールの`output`ポートかは検査しない（エラボレーションで検証する）。

`is_keyword(s: &str) -> bool` :

- 概要: 文字列がキーワード（`var` / `bit` / `clock` / `module` / `port` / `input` / `output` / `testbench` / `initial` / `step`）かどうかを判定する。
- 背景: `grammar.pest` の `ident` ルールはキーワードを構文上区別しない（`var`/`bit`なども識別子として受理できてしまう）ため、識別子を確定させる各箇所（`parse_decl`・`parse_stmt`・`parse_expression_factor`・`parse_module_def`・`parse_port_decl`・`parse_inst_decl`・`parse_field_access`・`parse_testbench_def`・`parse_proc_assign`）でこの関数を呼んでキーワードを弾き、変数名・モジュール名・ポート名・インスタンス名・テストベンチ名としての使用を防いでいる。

### 文法ファイル: `grammar.pest`

- 場所: `src/grammar.pest`
- 役割: PEG 文法の定義ファイル。`pest_derive` がビルド時に読み込んで `CeriteParser` を生成する。
- 内容:

```pest
program = { SOI ~ (module_def | testbench_def)+ ~ EOI }

module_def = { "module" ~ ident ~ "{" ~ port_block ~ (decl | stmt)* ~ "}" }
port_block = { "port" ~ "{" ~ port_decl* ~ "}" }
port_decl  = { ident ~ ":" ~ direction ~ signal_type ~ ";" }
direction  = { "input" | "output" }
signal_type = { "clock" | ("bit" ~ ("<" ~ number ~ ">")?) }

testbench_def = { "testbench" ~ ident ~ "{" ~ (decl | inst_decl | stmt)* ~ initial_block? ~ "}" }
initial_block = { "initial" ~ "{" ~ (proc_assign | proc_step)* ~ "}" }
proc_assign   = { ident ~ "=" ~ ternary_expr ~ ";" }
proc_step     = { "step" ~ ";" }

decl      = { "var" ~ ident ~ ":" ~ signal_type ~ ";" }
inst_decl = { "var" ~ ident ~ "=" ~ ident ~ "(" ~ (named_arg ~ ("," ~ named_arg)*)? ~ ")" ~ ";" }
named_arg = { ident ~ ":" ~ ternary_expr }
stmt      = { ident ~ (assign | nonblock) ~ ternary_expr ~ ";" }
assign    = { "=" }
nonblock  = { "<=" }
ternary_expr      = { expression ~ ("?" ~ ternary_expr ~ ":" ~ ternary_expr)? }
expression        = { expression1 ~ (or_op ~ expression1)* }
expression1       = { expression2 ~ (and_op ~ expression2)* }
expression2       = { expression3 ~ (bitor_op ~ expression3)* }
expression3       = { expression4 ~ (xor_op ~ expression4)* }
expression4       = { expression5 ~ (bitand_op ~ expression5)* }
expression5       = { expression6 ~ (eq_op ~ expression6)* }
expression6       = { expression7 ~ (rel_op ~ expression7)* }
expression7       = { expression8 ~ (shift_op ~ expression8)* }
expression8       = { expression9 ~ (add_op ~ expression9)* }
expression9       = { expression_unary ~ (mul_op ~ expression_unary)* }
expression_unary  = { unary_op* ~ expression_factor }
expression_factor = { field_access | ident | bitvec_literal | number | "(" ~ ternary_expr ~ ")" }
field_access      = { ident ~ "." ~ ident }

or_op     = { "||" }
and_op    = { "&&" }
bitor_op  = { "|" }
xor_op    = { "^" }
bitand_op = { "&" }
eq_op     = { "==" | "!=" }
rel_op    = { "<=" | "<" | ">=" | ">" }
shift_op  = { "<<<" | "<<" | ">>>" | ">>" }
add_op    = { "+" | "-" }
mul_op    = { "*" | "/" | "%" }
unary_op  = { "!" | "~" }

ident   = @{ (ASCII_ALPHA | "_") ~ (ASCII_ALPHANUMERIC | "_")* }
number  = @{ ASCII_DIGIT+ }

bitvec_literal = ${ number ~ "'" ~ radix ~ literal_digits }
radix          = { "b" | "o" | "d" | "h" }
literal_digits = @{ ASCII_ALPHANUMERIC+ }

WHITESPACE = _{ " " | "\t" | "\r" | "\n" }
COMMENT    = _{ "//" ~ (!"\n" ~ ANY)* }
```

- 演算子は個別ルール（`or_op`/`eq_op`など）でラップしている。pestでは無名の文字列リテラルは子Pairにならないため、`("+" | "-")`のように選択肢が複数ある演算子はラップしないとどちらが一致したか判別できない。
- 各優先順位ルールは `(op ~ next)*` の繰り返し形にしている。単に `next ~ op ~ next`（1回だけ）にすると、演算子を含まない単項の式や3項以上の連鎖がパースできなくなる。
- `rel_op`/`shift_op` は選択肢の順序が重要。PEGの選択はバックトラックせず最初に一致したものを採用するため、`"<"` を `"<="` より先に書くと `<=` の `=` が読めずに壊れる。長い演算子を先に置く必要がある（例: `"<="` の後に `"<"`）。
- `unary_op`（前置の `!`）と `eq_op` の `!=` は文字が重なるが衝突しない。`unary_op` は「オペランドの開始位置」（`expression_unary` の先頭）でのみ試され、`!=` は「演算子の開始位置」（`eq_op` の位置、両オペランドの間）でのみ試されるため、同じ入力位置で競合することがない。
- `ternary_expr` は二項演算子チェーン全体（`expression`）よりさらに外側にある。`stmt` の右辺と `expression_factor` の括弧の中身は、どちらも `expression` ではなく `ternary_expr` を参照することで、`a ? b : c` だけでなく `(a ? b : c) + 1` のように括弧内でも三項演算子を使えるようにしている。右結合にするため、then/else 側の再帰先はどちらも `ternary_expr` 自身（1つ下の優先順位ではなく自分自身）にしている。
- `bitvec_literal` は `${ ... }`（compound-atomic）で定義している。`@{ ... }`（atomic）だと内部の `number`/`radix`/`literal_digits` の子Pairが消えて丸ごと1つのトークンになってしまい、幅・基数・桁を個別に取り出せなくなる。`${ ... }` は空白の暗黙挿入を止めつつ（`4 'b 1010` のような書き方を防ぐ）、子Pairは維持してくれる。
- `signal_type` は `port_decl`/`decl`で共用する。`reg`/`wire`のような宣言キーワードは無く、あくまで`bit`/`bit<N>`/`clock`という値の型のみを表す。`reg`相当（レジスタ）かどうかは代入演算子（`=`/`<=`）から`netlist`が自動的に決定する（後述）。`clock`型を`var`宣言できるのはテストベンチ内のみ・`output`ポートには使えない、という制約は文法ではなく`elaboration`で検証する。
- `expression_factor` では `bitvec_literal` を `number` より先に置いている。`4'b1010` の `4` の部分だけで `number` にマッチしてしまうと、続く `'b1010` が余ってパース全体が失敗する。PEGの順序付き選択では `bitvec_literal` を先に試し、`'` が続かない入力（例えば単なる `42`）では自動的にバックトラックして `number` にフォールバックする。
- 同様に `field_access`（`ident ~ "." ~ ident`）も `expression_factor` の中で単独の `ident` より先に置いている。`u1.sum` の `u1` だけで `ident` にマッチしてしまうと `.sum` が余ってパースが壊れるため、`field_access` を先に試し、`.` が続かない入力では `ident` にバックトラックする。
- `inst_decl`（モジュールインスタンス化）は `testbench_def` の中にのみあり、`module_def` の本体（`(decl | stmt)*`）には含まれていない。これにより「モジュールが別のモジュールをインスタンス化する」というネストが文法レベルで禁止されている（現状の制限。将来ネストに対応する場合はここを緩める）。
- `testbench_def` の `(decl | inst_decl | stmt)*` の直後に `initial_block?` を続けている。`initial` はキーワードなので `stmt`（`ident ~ (assign | nonblock) ~ ...`）が「initial」を識別子として食おうとしても、続く `{` が `assign`/`nonblock`（`=`/`<=`）にマッチせず `stmt` の試行は失敗し、`decl`/`inst_decl` も `"initial"` では始まらないため `(decl | inst_decl | stmt)*` はそこで自然に止まり、`initial_block` の解析に移る。
- `program` は `SOI ~ ... ~ EOI` で入力全体の消費を明示的に要求している。pestの`Parser::parse()`は、指定したルールが入力の**先頭部分**にさえ一致すれば成功を返し、末尾に残った未消費の入力があってもエラーにしない（`EOI`を明示しない限り）。これを`SOI`/`EOI`無しのまま放置すると、例えば`//`コメントのようにこの文法がサポートしていない構文が入力の途中に現れた場合、そこで静かにパースを打ち切り、それ以降の内容（コメントの後に続くはずのブロック全体など）を**エラーも警告も無く**捨ててしまう（実際にこの不備が原因で、コメント付きのソースの後半ブロックが丸ごと無視されるバグが起きたことがある）。`EOI`まで明示的に要求することで、こうした未消費の残りは即座にパースエラーになる。
- `COMMENT`（`"//" ~ (!"\n" ~ ANY)*`）は行コメントを定義する silent ルール。`WHITESPACE`と同様、`COMMENT`という名前の非atomicルールは暗黙的に他のルールのトークン間（`~`の間）に挿入されるため、個々のルールで明示的にコメントを許可する記述は不要。

- `~` が連接、`|` が選択、`*` が0回以上の繰り返し、`?` が0回または1回、`()` がグループ化
- `@{ ... }` はアトミックルール（内部で WHITESPACE をスキップしない）
- `_{ ... }` は silent ルール（AST に現れない）
- `WHITESPACE`/`COMMENT` は暗黙的に他のルールのトークン間に挿入される特殊ルール
- `SOI`/`EOI` は入力の先頭/末尾を表す組み込みルール

---

## `elaboration` モジュール

モジュール定義とトップレベルは、どちらも「信号・代入文・インスタンスの集合」という共通の形（`ResolvedScope`）で扱う。モジュール定義は**宣言された時点で（インスタンス化の有無によらず）1回だけ**解決・検証され、インスタンス化のたびに使い回される。階層はここではまだ展開されない（展開するのは`netlist`モジュール）。

### 型

`ElabError` :

- 役割: エラボレーションエラー（未宣言変数、重複宣言など）。
- フィールド:
  - `message: String` — 日本語のエラーメッセージ

`ResolvedSignal` :

- 役割: 解決済みの信号定義。パース時の `Decl`（またはポート宣言）から変数名をシンボルテーブルで ID に変換したもの。
- フィールド:
  - `name: String` — 変数名（元のソースの名前）
  - `width: u64` — ビット幅（`bit` = 1、`bit<N>` = N、`clock` = 1）
  - `id: usize` — 信号ID（そのスコープ内で0始まりの通番。モジュール本体とトップレベルはそれぞれ別のID空間を持つ）
  - `is_clock: bool` — `clock`型として宣言されたか。モジュールインスタンス化時の接続の型検査（`resolve_instance_connections`）に使う

`ResolvedStmt` (enum) :

- 役割: 解決済みの代入文。変数名が ID に置き換わっている。
- バリアント:
  - `Combinational { target_id: usize, expr: ResolvedExpr }` — 組み合わせ代入
    - `target_id` — 代入先信号のID
    - `expr` — 右辺の解決済み式
  - `Sequential { target_id: usize, expr: ResolvedExpr }` — 順序代入

`ResolvedExpr` (enum) :

- 役割: 解決済みの式。変数参照が ID に置き換わっている。
- バリアント:
  - `Ident(usize)` — 信号ID参照
  - `Number(u64)` — 数値リテラル
  - `BitVecLiteral { width: u64, value: u64 }` — ビットベクタリテラル
  - `InstanceField { instance_name: String, port_name: String }` — モジュールインスタンスの出力ポート参照（`u1.sum`の解決後）。この時点ではまだ「同じスコープのどのインスタンスのどのポートか」という名前のままで、実際のノードへの変換は`netlist`モジュールが階層を展開するタイミングで行う
  - `BinOp { op: BinOp, lhs: Box<ResolvedExpr>, rhs: Box<ResolvedExpr> }` — 二項演算
  - `UnaryOp { op: UnOp, expr: Box<ResolvedExpr> }` — 前置単項演算
  - `Ternary { cond: Box<ResolvedExpr>, then_branch: Box<ResolvedExpr>, else_branch: Box<ResolvedExpr> }` — 三項演算（条件式）

`ResolvedPort` :

- 役割: 解決済みのポート定義。
- フィールド:
  - `name: String` — ポート名
  - `direction: Direction` — 向き
  - `signal_id: usize` — モジュール本体内でのローカル信号ID（`body.signals`のインデックスと対応）
  - `is_clock: bool` — `clock`型ポートか（`resolve_instance_connections`が接続の型検査に使う）

`ResolvedInstance` :

- 役割: 解決済みのモジュールインスタンス。
- フィールド:
  - `instance_name: String` — インスタンス名
  - `module_name: String` — インスタンス化したモジュール名
  - `connections: HashMap<String, ResolvedExpr>` — ポート名 → 接続式（**呼び出し側スコープの信号IDで**解決済み）のマップ

`ResolvedScope` :

- 役割: 信号・代入文・インスタンスの集合。トップレベルにもモジュール本体にも使う共通の形。
- フィールド:
  - `signals: Vec<ResolvedSignal>` — このスコープの信号のリスト
  - `stmts: Vec<ResolvedStmt>` — このスコープの代入文のリスト
  - `instances: Vec<ResolvedInstance>` — このスコープが直接持つモジュールインスタンスのリスト（モジュール本体は現状ネスト不可のため常に空）

`ResolvedModuleDef` :

- 役割: 解決済みのモジュール定義。
- フィールド:
  - `name: String` — モジュール名
  - `ports: Vec<ResolvedPort>` — ポート定義のリスト
  - `body: ResolvedScope` — モジュール本体（ポートも通常の信号として`body.signals`に含まれる）
  - `clock_port: Option<usize>` — `clock`型入力ポートのローカル信号ID（`resolve_module_ports`が高々1つに限定して検出する。無ければ`None`）。`netlist`がインスタンスごとにregの`SignalKind::Reg.clock`を組み立てる際に使う

`ResolvedProcStmt` (enum) :

- 役割: 解決済みの手続き文（`initial { }` 内）。
- バリアント:
  - `Assign { target_id: usize, expr: ResolvedExpr }` — 対象信号ID（トップレベルのシンボルテーブルで解決済み）に値を設定する
  - `Step` — シミュレーションを1サイクル進める

`Elaborated` :

- 役割: エラボレーション結果全体。
- フィールド:
  - `top: ResolvedScope` — トップレベルのスコープ（テストベンチの並行部分もここに合流している）
  - `modules: HashMap<String, ResolvedModuleDef>` — モジュール名 → 解決済みモジュール定義
  - `initial: Vec<ResolvedProcStmt>` — テストベンチの`initial`（手続き部分）の解決後リスト（テストベンチが無い、または`initial`が無ければ空）

`SymbolTable` (type alias) :

- 定義: `HashMap<String, usize>`
- 役割: 変数名 → 信号ID のマッピング。1つのスコープの中で一時的に構築される。

`InstanceTable` (type alias) :

- 定義: `HashMap<String, String>`
- 役割: インスタンス名 → モジュール名 のマッピング。`Expr::FieldAccess`（`u1.sum`）を解決する際、`u1`がどのモジュールのインスタンスかを引くために使う。

`WHITE` / `GRAY` / `BLACK` (定数, `u8`) :

- 役割: `dfs_visit` のDFS色付け（未訪問/探索中/探索済み）に使う定数。`check_combinational_loops` と `dfs_visit` の双方から参照するためファイルスコープに定義されている。

### 関数

`elaborate(prog: &Program) -> Result<Elaborated>` :

- 概要: AST を受け取り、まずモジュール定義をすべて解決・検証し、そのあとトップレベルを解決する。最後にテストベンチの`initial`を解決する。
- 処理:
  1. `prog.testbenches.len() > 1` ならエラー（テストベンチは高々1つ）
  2. `build_module_defs` で全モジュール定義を1回ずつ解決・検証
  3. `elaborate_top` でトップレベル（テストベンチの並行部分）を解決し、あわせて`SymbolTable`/`InstanceTable`を得る
  4. `resolve_initial` で、3で得た`SymbolTable`/`InstanceTable`を使ってテストベンチの`initial`を解決する
  5. `Elaborated { top, modules, initial }` を返す

`build_module_defs(prog: &Program) -> Result<HashMap<String, ResolvedModuleDef>>` :

- 概要: `prog.modules` を走査し、モジュール名の重複チェックをしながら各モジュールを `resolve_module_def` で解決する。

`resolve_module_def(m: &ModuleDef) -> Result<ResolvedModuleDef>` :

- 概要: 1つのモジュール定義を解決・検証する。ポートも内部信号もこの関数のローカルなシンボルテーブルに登録され、この関数の中で完結する（インスタンス化の回数に関わらず1回だけ実行される）。処理自体は`resolve_module_ports`・`resolve_module_decls`・`resolve_module_stmts`の3つの補助関数に分割されており、`resolve_module_def`はそれらを順に呼び出して静的チェックをかけるだけの薄い関数になっている。
- 処理:
  1. `resolve_module_ports`でポートを信号として登録し、信号リスト・シンボルテーブル・`ResolvedPort`リスト・`clock_port`を得る
  2. `resolve_module_decls`で内部の`var`宣言を同じ信号リスト・シンボルテーブルに追加登録（1のポート用シンボルテーブルを可変参照で受け取り、そのまま拡張する）
  3. `resolve_module_stmts`で本体の代入文を解決
  4. `clock_port`が`None`かつ`stmts`に`Sequential`が1つでもあればエラー（reg使用時はclock型入力ポートが必須）
  5. `check_multiple_drivers`・`check_combinational_loops` を適用（`input`ポートは代入先になり得ないため、これらの関数自体に変更は不要）
  6. `ResolvedModuleDef { name, ports, body: ResolvedScope { signals, stmts, instances: vec![] }, clock_port }` を返す

`resolve_module_ports(m: &ModuleDef) -> Result<(Vec<ResolvedSignal>, SymbolTable, Vec<ResolvedPort>, Option<usize>)>` :

- 概要: モジュールのポート宣言（`m.ports`）を信号として登録する（重複ポート名はエラー）。`ResolvedPort`（向き・ローカル信号ID・`is_clock`）も同時に構築し、`clock`型入力ポートのローカル信号IDを`clock_port`として返す。
- 処理: 各ポートについて、`clock`型かつ`output`ならエラー。`clock`型の`input`ポートが既に見つかっている状態でもう1つ見つかったらエラー（高々1つに限定）。

`resolve_module_decls(m: &ModuleDef, signals: &mut Vec<ResolvedSignal>, symtab: &mut SymbolTable) -> Result<()>` :

- 概要: モジュール本体の`var`宣言（`m.decls`）を、`resolve_module_ports`が作った信号リスト・シンボルテーブルに追加登録する（重複宣言はエラー）。ポートと内部信号が同じフラットな信号空間に合流する。`clock`型の`var`宣言は常にエラー（`clock`型の`var`宣言はテストベンチ内でのみ許可される）。

`resolve_module_stmts(m: &ModuleDef, symtab: &SymbolTable, ports: &[ResolvedPort]) -> Result<Vec<ResolvedStmt>>` :

- 概要: モジュール本体の代入文（`m.stmts`）を解決する。代入先が`input`ポートの場合はエラー（`input`は外部から供給されるため内部で駆動できない）。式の解決には`resolve_expr`を使うが、モジュール本体は現状インスタンスを持てないため、空の`InstanceTable`と空のモジュールテーブルを渡す。

`elaborate_top(prog: &Program, modules: &HashMap<String, ResolvedModuleDef>) -> Result<(ResolvedScope, SymbolTable, InstanceTable)>` :

- 概要: トップレベル（テストベンチの並行部分）を解決する。後段の`resolve_initial`が同じシンボルテーブル・インスタンステーブルを再利用できるよう、`ResolvedScope`と一緒に返す。
- 処理:
  1. `build_signals` で信号を解決
  2. `build_instances` でモジュールインスタンス化を解決（モジュールテーブルを参照）
  3. `resolve_stmts` で代入文を解決（インスタンステーブルも渡し、`FieldAccess`の解決に使う）
  4. `check_multiple_drivers`・`check_combinational_loops` を適用

`resolve_initial(prog, symtab, instances, modules) -> Result<Vec<ResolvedProcStmt>>` :

- 概要: テストベンチの`initial`（手続き文）を解決する。対象信号はトップレベルのシンボルテーブルで解決する（`initial`はモジュール本体を持てないため、参照できるのはトップレベルの信号とインスタンスのみ）。
- 処理: `prog.testbenches`の各`initial`ステップを走査し、`ProcStmt::Assign { target, expr }` → 対象変数名をシンボルテーブルで ID に解決（未宣言ならエラー）、式を`resolve_expr`で解決して`ResolvedProcStmt::Assign`に、`ProcStmt::Step` → `ResolvedProcStmt::Step`にそのまま変換。

`build_signals(prog: &Program) -> Result<(Vec<ResolvedSignal>, SymbolTable)>` :

- 概要: テストベンチの並行部分の宣言を走査し、シンボルテーブルと解決済み信号リストを構築する（トップレベルの信号空間はここでのみ構築される）。1件ずつの登録は`push_signal`に切り出されている。
- 処理: `prog.testbenches`の`decls`を走査し、それぞれ`push_signal`で登録する。

`push_signal(signals: &mut Vec<ResolvedSignal>, symtab: &mut SymbolTable, decl: &Decl) -> Result<()>` :

- 概要: 1つの`decl`を信号として登録する（重複宣言はエラー）。シンボルテーブル（名前→ID）の構築、`ResolvedSignal`（`width`/`is_clock`は`decl.sig_type`から決まる）の追加を行う。

`build_instances(prog: &Program, symtab: &SymbolTable, signals: &[ResolvedSignal], modules: &HashMap<String, ResolvedModuleDef>) -> Result<(Vec<ResolvedInstance>, InstanceTable)>` :

- 概要: テストベンチの並行部分のモジュールインスタンス化を走査し、引数をポート定義と突き合わせて解決する。1件ずつの検査・解決は`check_instance_name_available`・`resolve_instance_connections`の2つの補助関数に切り出されており、`build_instances`自体はループを回してインスタンステーブルを育てながら結果を集約するだけになっている。
- 処理: 各インスタンス化について、`check_instance_name_available`でインスタンス名の重複を検査 → 参照するモジュールが定義されているか検査 → `resolve_instance_connections`で引数をポート定義と突き合わせて解決 → インスタンステーブル（`instance_table`と、後続の接続式解決からも見える`resolved_so_far`の両方）に登録し、`ResolvedInstance`を積み上げる。`signals`は接続式の型検査（`clock`型かどうか）のために`resolve_instance_connections`へそのまま渡される。

`check_instance_name_available(name: &str, symtab: &SymbolTable, instance_table: &InstanceTable) -> Result<()>` :

- 概要: インスタンス名が信号名・既存のインスタンス名のどちらとも重複していないか検査する（重複していればエラー）。

`resolve_instance_connections(inst: &InstDecl, module_def: &ResolvedModuleDef, symtab: &SymbolTable, signals: &[ResolvedSignal], resolved_so_far: &InstanceTable, modules: &HashMap<String, ResolvedModuleDef>) -> Result<HashMap<String, ResolvedExpr>>` :

- 概要: 1つのインスタンス化の引数（名前付き接続式）を、対象モジュールの入力ポート定義と突き合わせて解決する。
- 処理: 各引数が実在する`input`ポート名か（`output`ポート名を指定した場合や存在しないポート名は専用のエラーメッセージ）、重複していないかを検査 → 引数式を`resolve_expr`で解決（`resolved_so_far`を渡すことで、この時点までに解決済みの同スコープの他インスタンスも参照できる）→ `resolved_expr_is_clock`で接続式が`clock`型信号への直接参照かどうかを判定し、ポートの`is_clock`と一致しなければエラー（`clock`⇔`clock`以外の組み合わせは不可）→ 全`input`ポート分の接続が揃っているか検査。

`resolved_expr_is_clock(expr: &ResolvedExpr, signals: &[ResolvedSignal]) -> bool` :

- 概要: 解決済み式が「`clock`型の信号への直接参照」かどうかを判定する。
- 処理: `ResolvedExpr::Ident(id)`かつ`signals[id].is_clock`のときのみ`true`。`clock`型は`output`ポートに使えないため`InstanceField`が`clock`型になることはなく、演算結果（`BinOp`/`UnaryOp`/`Ternary`など）も常に`clock`型ではない。

`resolve_stmts(prog, symtab, instances, modules) -> Result<Vec<ResolvedStmt>>` :

- 概要: テストベンチの並行部分の代入文を走査し、変数名をシンボルIDに解決する。
- 処理: 代入先の変数名をシンボルテーブルで ID に解決（未宣言ならエラー）、右辺の式を再帰的に解決（`resolve_expr`。インスタンステーブルとモジュールテーブルも渡す）、代入の種類（Combinational/Sequential）を保持

`check_multiple_drivers(stmts: &[ResolvedStmt], signals: &[ResolvedSignal]) -> Result<()>` :

- 概要: 同一信号への複数ドライバ（多重代入）を検出する。モジュール本体・トップレベルどちらの`ResolvedScope`にも同じ関数を使う。
- 処理: `HashSet` に `target_id` を挿入していき、既に挿入済みの ID が再度出てきたらエラー（信号名は `signals[target_id].name` から引く）

`resolve_expr(expr: &Expr, symtab: &SymbolTable, instances: &InstanceTable, modules: &HashMap<String, ResolvedModuleDef>) -> Result<ResolvedExpr>` :

- 概要: AST の式を再帰的に解決済み式に変換する。モジュール本体を解決する際は`instances`に空のマップを渡すため、`FieldAccess`が現れても「インスタンスが見つからない」エラーに自然に倒れる（現状モジュール本体はインスタンスを持てないため、この経路は文法上そもそも通らない）。
- 処理:
  - `Ident(name)` → シンボルテーブルで ID に解決
  - `Number(n)` → そのまま
  - `BitVecLiteral { width, value }` → そのまま（信号参照を含まないため解決不要）
  - `FieldAccess { instance, field }` → `instances`でインスタンス名からモジュール名を引き、`modules`でそのモジュールの定義を引く。`field`が実在するポート名か、かつ`output`ポートかを検査（`input`ポートを外部から読もうとするとエラー）した上で`ResolvedExpr::InstanceField`に変換
  - `BinOp { op, lhs, rhs }` → 左右を再帰解決して `ResolvedExpr::BinOp`
  - `UnaryOp { op, expr }` → オペランドを再帰解決して `ResolvedExpr::UnaryOp`
  - `Ternary { cond, then_branch, else_branch }` → 3つとも再帰解決して `ResolvedExpr::Ternary`

`check_combinational_loops(stmts: &[ResolvedStmt], signals: &[ResolvedSignal]) -> Result<()>` :

- 概要: 組合せ代入（Combinational）だけを対象に依存グラフを作り、循環がないか検査する。順序代入（Sequential）は1サイクル遅れて反映されるため依存グラフに含めない（循環があってもループにならない）。
- 処理:
  1. `build_combinational_deps` で依存グラフを構築
  2. 全信号を色 `WHITE` で初期化
  3. 未訪問（`WHITE`）の信号ごとに `dfs_visit` を呼ぶ
- **既知の制限**: `ResolvedExpr::InstanceField`（`u1.sum`のような読み出し）は`collect_read_signals`で依存なしの葉として扱われる。そのため、あるインスタンスの出力を同じインスタンス（または相互依存する複数インスタンス）の入力に戻すような、**インスタンス境界をまたぐ組合せループはここでは検出できない**。これは、モジュール定義ごとに「どの`input`ポートがどの`output`ポートに組合せ的に影響するか」という依存関係の要約を計算し、インスタンス化のたびに呼び出し側の依存グラフへ合成する必要があるためで、今回のモジュール対応の初期実装ではスコープ外にしている。実際にそのような回路を書いた場合、ネットリスト構築で得られるフラットなDAG自体には循環が残るため、シミュレーション実行時に`Simulator::step`のΔ-サイクル上限（`MAX_COMB_ITERATIONS`）で検出され、パニックになる（コンパイル時ではなく実行時の、より遅いタイミングでの検出になる）。

`build_combinational_deps(stmts: &[ResolvedStmt], signal_count: usize) -> Vec<Vec<usize>>` :

- 概要: 組合せ代入の依存グラフを構築する。
- 処理: `deps[信号ID] = その信号を右辺で読む Combinational Drive のターゲットID一覧`（`collect_read_signals` で各文の右辺から読み取り信号を収集）

`dfs_visit(node: usize, deps: &[Vec<usize>], color: &mut [u8], path: &mut Vec<usize>, signals: &[ResolvedSignal]) -> Result<()>` :

- 概要: 依存グラフをDFSで訪問し、循環を検出する（経路を `path` に保持する）。
- 処理:
  1. 白（未訪問）・灰（探索中）・黒（探索済み）で色付けしながら再帰的にDFS
  2. 探索中（灰）のノードに戻る辺を見つけたら `cycle_error` でエラーを組み立てて返す
  3. 未訪問（白）のノードへは再帰的に `dfs_visit` を呼ぶ

`cycle_error(path: &[usize], next: usize, signals: &[ResolvedSignal]) -> ElabError` :

- 概要: `dfs_visit` が循環を検出した際、経路を含むエラーメッセージを組み立てる。
- 処理: `path` から循環の開始位置を探し、そこから `next` までの信号名を `→` で連結してメッセージ化

`collect_read_signals(expr: &ResolvedExpr) -> Vec<usize>` :

- 概要: 解決済み式が右辺で参照している信号IDを再帰的に集める（`build_combinational_deps` の依存グラフ構築に使用）。
- 処理:
  - `Ident(id)` → `[id]`
  - `Number(_)` → 空
  - `BitVecLiteral { .. }` → 空
  - `InstanceField { .. }` → 空（上記「既知の制限」を参照）
  - `BinOp { lhs, rhs, .. }` → 左右を再帰収集して連結
  - `UnaryOp { expr, .. }` → オペランドを再帰収集
  - `Ternary { cond, then_branch, else_branch }` → 3つとも再帰収集して連結

---

## `netlist` モジュール

### 型

`NodeId` (type alias) :

- 定義: `usize`
- 役割: ネットリストノードの識別子（`nodes` ベクタのインデックス）

`Node` (enum) :

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
  - `UnaryOp { id: NodeId, op: UnOp, operand: NodeId, width: u64 }` — 単項演算
    - `op` — 演算子
    - `operand` — オペランドのノードID
    - `width` — 結果のビット幅（`Not`なら1、`BitNot`ならオペランドと同じ幅）
  - `Ternary { id: NodeId, cond: NodeId, then_branch: NodeId, else_branch: NodeId, width: u64 }` — 三項演算（条件式）
    - `cond` — 条件のノードID
    - `then_branch` — 条件が真の場合のノードID
    - `else_branch` — 条件が偽の場合のノードID
    - `width` — 結果のビット幅（then/elseの大きい方）
  - `Drive { id: NodeId, signal_id: usize, signal_name: String, source: NodeId, kind: DriveKind, width: u64 }` — 信号駆動
    - `signal_id` — 駆動する信号のID
    - `signal_name` — デバッグ用の信号名
    - `source` — 駆動値のソースノードID
    - `kind` — 駆動の種類（Combinational/Sequential）
    - `width` — 駆動先信号のビット幅（`Simulator::step`が代入時のマスキングに使う）

`DriveKind` (enum) :

- 役割: 信号駆動の種類。
- バリアント:
  - `Combinational` — 組み合わせ（`=`、即時反映）
  - `Sequential` — 順序（`<=`、サイクル終了時に一斉反映）

- `Display` 実装: `Combinational` → `"blocking"`, `Sequential` → `"non-blocking"`

`Edge` (enum) :

- 役割: クロック/リセットのエッジの向き。
- バリアント: `Posedge`（立ち上がり） `Negedge`（立ち下がり）

`ClockTrigger` :

- 役割: reg更新やリセットのトリガーとなる信号とエッジ。
- フィールド:
  - `signal_id: usize` — トリガーとなる信号のID
  - `edge: Edge` — トリガーとするエッジ

`ResetSpec` :

- 役割: regのリセット仕様。
- フィールド:
  - `trigger: ClockTrigger` — リセットのトリガー
  - `value: u64` — リセット時に設定する値

`SignalKind` (enum) :

- 役割: 信号の種別（wire/reg）。`reg`/`wire`を区別する宣言キーワードは無く、`kind`自体は既存の代入演算子（`=`/`<=`）から`flatten_scope`が自動的に決定する（`Combinational`駆動なら`Wire`、`Sequential`駆動なら`Reg`）。
- バリアント:
  - `Wire` — 組み合わせ駆動、または未駆動の信号
  - `Reg { clock: Option<ClockTrigger>, reset: Option<ResetSpec> }` — 順序駆動の信号
    - `clock` — 更新のトリガー。そのregがモジュール本体に属し、かつそのモジュールに`clock`型入力ポートがあれば`Some(ClockTrigger { signal_id: <グローバル化したclock_port>, edge: Edge::Posedge })`（`edge`は暫定的にposedge固定、negedge/両エッジは未サポート）。トップレベル/テストベンチ直下のreg（モジュールに属さない）は常に`None`
    - `reset` — リセットの仕様（現状常に`None`、先行実装のみで未使用）

`InitialStep` (enum) :

- 役割: テストベンチの`initial { }`の手続き文をネットリスト向けに展開したもの。
- バリアント:
  - `Assign { target: usize, expr_node: NodeId }` — 対象信号（グローバルID）を、式ノードを評価した値でその場で設定する（`Drive`ノードは経由しない、継続的な駆動ではない一度きりの設定）
  - `Step` — シミュレーションを1サイクル進める

`Netlist` :

- 役割: 生成されたネットリスト全体。
- フィールド:
  - `signals: Vec<NetlistSignal>` — 全信号のリスト
  - `nodes: Vec<Node>` — 全ノードのリスト（DAG の頂点集合）
  - `initial: Vec<InitialStep>` — テストベンチの`initial`手続きの実行手順（テストベンチが無い、または`initial`が無ければ空）

`NetlistSignal` :

- 役割: ネットリスト上の信号情報。
- フィールド:
  - `id: usize` — 信号ID
  - `name: String` — 信号名
  - `width: u64` — ビット幅
  - `driver_node: Option<NodeId>` — この信号を駆動する Drive ノードのID（未駆動 = None）
  - `driver_kind: Option<DriveKind>` — 駆動の種類（未駆動 = None）
  - `kind: SignalKind` — 信号の種別（wire/reg）。`Combinational`駆動または未駆動なら`Wire`、`Sequential`駆動なら`Reg { clock, reset: None }`（`clock`はそのregが属すモジュールの`clock`型入力ポートに紐付く。無ければ`None`）

`InstanceRemaps` (type alias) :

- 定義: `HashMap<String, (String, Vec<usize>)>`
- 役割: スコープ内のインスタンス名 → `(モジュール名, ローカル信号ID→グローバル信号IDのリマップ)`。`flatten_scope`が自分の直下のインスタンスを展開するたびに1件ずつ育て、同じ呼び出しの中で`build_expr`が`InstanceField`を解決する際に参照する。

`NetlistBuilder` :

- 役割: 内部ビルダー。モジュール階層を`flatten_scope`で再帰的に辿りながら、フラットな`signals`/`nodes`を構築する。
- フィールド:
  - `nodes: Vec<Node>` — 構築中のノードリスト
  - `signals: Vec<NetlistSignal>` — 構築中の信号リスト（展開されたすべてのスコープの信号がここに集まる）
  - `next_id: NodeId` — 次に割り当てるノードID

- メソッド:
  - `new() -> Self` — 空のビルダーを生成
  - `alloc_id(&mut self) -> NodeId` — 新しいノードIDを割り当てる
  - `add_node(&mut self, node: Node) -> NodeId` — ノードを追加し、そのIDを返す
  - `make_const(&mut self, value, width) -> NodeId` — 定数ノードを生成
  - `make_read_signal(&mut self, signal_id, name, width) -> NodeId` — 信号読み出しノードを生成
  - `make_binop(&mut self, op, lhs, rhs, width) -> NodeId` — 二項演算ノードを生成
  - `make_unaryop(&mut self, op, operand, width) -> NodeId` — 単項演算ノードを生成
  - `make_ternary(&mut self, cond, then_branch, else_branch, width) -> NodeId` — 三項演算ノードを生成
  - `make_drive(&mut self, signal_id, name, source, kind, width) -> NodeId` — 駆動ノードを生成
  - `drive_signal(&mut self, target_global, source, kind)` — `target_global`（グローバル信号ID）を`make_drive`で駆動し、対応する`NetlistSignal`の`driver_node`/`driver_kind`を更新する共通ヘルパー（組み合わせ代入・順序代入・インスタンスの`input`ポート接続の3箇所から呼ばれる）
  - `flatten_scope(&mut self, scope: &ResolvedScope, prefix: &str, modules, clock_port: Option<usize>) -> (Vec<usize>, InstanceRemaps)` — スコープ（トップレベルまたはモジュール本体）を再帰的にフラット化する。`clock_port`はこのスコープがモジュール本体の場合そのモジュールの`clock_port`（トップレベルは常に`None`）。詳細は下記
  - `build_expr(&mut self, expr, remap, instance_remaps, modules) -> NodeId` — 解決済み式からノードを構築（`BinOp`の結果幅は、`Or`/`And`/`Eq`/`Neq`/`Lt`/`Le`/`Gt`/`Ge`なら1ビット、それ以外は両オペランドの大きい方。`UnaryOp`の結果幅は、`Not`なら1ビット、`BitNot`ならオペランドと同じ幅。`Ternary`の結果幅は`then_branch`/`else_branch`の大きい方、`cond`は幅に影響しない。`BitVecLiteral`は専用の`Node`を持たず`make_const`にそのまま渡すが、明示された幅に収まらない値はここで幅へ切り詰めてから渡す。`remap`は現在のスコープのローカル信号ID→グローバル信号IDのリマップ、`instance_remaps`は現在のスコープが直接持つインスタンスのリマップで、`Ident`/`InstanceField`の解決にそれぞれ使う）
  - `node_width(&self, node_id) -> u64` — ノードのビット幅を取得

### 関数

`scoped_name(prefix: &str, name: &str) -> String` :

- 概要: 信号名に名前空間プレフィックスを付ける。`prefix`が空文字列（トップレベル）ならそのまま、そうでなければ`"{prefix}.{name}"`（例: `"u1.sum"`）を返す。

`build_netlist(elab: &Elaborated) -> Netlist` :

- 概要: エラボレーション結果からネットリストを生成する。
- 処理:
  1. `NetlistBuilder::flatten_scope`をトップレベルスコープ（`elab.top`、プレフィックス空文字列、`clock_port: None`）に対して1回呼び、`(remap, instance_remaps)`を得る（モジュール階層はここで再帰的にフラット化される）
  2. `elab.initial`の各`ResolvedProcStmt`を走査し、`Assign { target_id, expr }`は1で得た`remap`/`instance_remaps`を使って式を`build_expr`し、`InitialStep::Assign { target: remap[target_id], expr_node }`に変換（`Drive`は生成しない）。`Step`はそのまま`InitialStep::Step`に変換
  3. `Netlist { signals, nodes, initial }` を返す
- 備考: モジュール階層はここで再帰的にフラット化される。展開後の`Node`/`NetlistSignal`はモジュールの存在を一切知らないため、`Simulator`はモジュール対応前と全く同じまま変更不要。`initial`のAssign式も普通の`build_expr`呼び出しで構築するだけなので、`InstanceField`（`u1.sum`など）を`initial`内で参照することもできる。

`NetlistBuilder::flatten_scope(&mut self, scope: &ResolvedScope, prefix: &str, modules: &HashMap<String, ResolvedModuleDef>, clock_port: Option<usize>) -> (Vec<usize>, InstanceRemaps)` :

- 概要: スコープ（トップレベルまたはモジュール本体）をフラットな信号・ノードへ再帰的に展開する。`clock_port`はこのスコープがモジュール本体の場合、そのモジュールの`clock`型入力ポートのローカル信号ID（トップレベルは常に`None`）。戻り値はこのスコープのローカル信号ID→グローバル信号IDのリマップと、このスコープが直接持つインスタンスのリマップ（呼び出し元がポート接続や、トップレベルなら`initial`の式構築に使う）。
- 処理:
  1. `scope.signals`の各信号を`scoped_name(prefix, ...)`で名前空間付きの`NetlistSignal`として`self.signals`に追加し、ローカルID→グローバルIDの`remap`を作る
  2. `scope.instances`の各インスタンスについて、そのモジュール本体を`prefix`にインスタンス名を足した名前空間（`scoped_name(prefix, instance_name)`）・そのモジュールの`clock_port`（`module_def.clock_port`）を渡して再帰的に`flatten_scope`し、`instance_remaps`に`(モジュール名, リマップ)`を記録。続けて、そのモジュールの`input`ポートそれぞれについて、接続式（外側スコープの`remap`で解決）を`build_expr`し、`drive_signal`でインスタンス内部の当該ポート信号を組み合わせDriveとして駆動する（インスタンス化 = 入力ポートへの合成の代入、という扱い）
  3. `scope.stmts`の各文について、`build_expr`（`remap`と、ここまでに構築した`instance_remaps`を渡す）→`drive_signal`で駆動。`Sequential`の場合はさらに、`clock_port`があれば`remap`でグローバル化した`ClockTrigger { signal_id, edge: Edge::Posedge }`（TODO: posedge固定は暫定処置）を、無ければ`None`を`clock`として`kind`を`SignalKind::Reg { clock, reset: None }`に更新
  4. `(remap, instance_remaps)`を返す

`format_netlist(nl: &Netlist) -> String` :

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
    N  2: BinOp(^)  (1 bit)  = N0 ^ N1
    N  3: Drive(a)  (blocking)  <= N2
    N  4: Read(a)  (1 bit)
    N  5: Drive(b)  (non-blocking)  <= N4
  ```
- `UnaryOp` の表示例（`x = !a;` の場合）: `N  2: UnaryOp(!)  (1 bit)  = !N1`
- `Ternary` の表示例（`x = a ? 1 : 0;` の場合）: `N  3: Ternary  (1 bit)  = N0 ? N1 : N2`

---

## `simulator` モジュール

### 型

`CycleSnapshot` :

- 役割: 1サイクル分のシミュレーション結果。
- フィールド:
  - `cycle: u64` — サイクル番号（0始まり）
  - `values: Vec<u64>` — 各信号の値（信号ID順、インデックス = 信号ID）

`Simulator` :

- 役割: ネットリストを評価して波形を生成する。
- フィールド:
  - `signal_values: Vec<u64>` — 現在の各信号の値（信号ID順）
  - `prev_signal_values: Vec<u64>` — 直前の`step`呼び出しの`settled`（1つ前のサイクルの組み合わせ収束後の値）。regのクロックエッジ検出に使う
  - `signal_kinds: Vec<SignalKind>` — 各信号の種別（wire/reg、およびregのクロック紐付け）を`NetlistSignal`から複製して持つ（`step`/`run`のシグネチャを変えずに`SignalKind`を参照できるようにするため）
  - `cycle: u64` — 経過サイクル数

### 関数

`Simulator::new(signals: &[NetlistSignal]) -> Self` :

- 概要: 全信号を0で初期化したシミュレーターを生成する。`signals`から`SignalKind`だけを複製して`signal_kinds`に保持する。
- 引数: `signals` — ネットリストの信号リスト（`clock`紐付けの参照に使うため、単なる信号数ではなく`NetlistSignal`そのものを受け取る）

`Simulator::set_signal(&mut self, id: usize, value: u64)` :

- 概要: 特定の信号に初期値を設定する（step() 実行前に呼ぶ）。

`Simulator::signal_values(&self) -> &[u64]` :

- 概要: 現在の全信号値をスライスで返す。

`Simulator::cycle(&self) -> u64` :

- 概要: 現在のサイクル数を返す。

`Simulator::step(&mut self, nodes: &[Node]) -> CycleSnapshot` :

- 概要: 1サイクル分シミュレーションを進め、結果のスナップショットを返す。
- 処理:
  1. スナップショット取得: サイクル開始時の全信号値（`snapshot`）をクローンする（ノンブロッキングの代入値評価に使う）
  2. Phase 1 — 組み合わせ評価（Δ-サイクル、最大1000回）:
     - 全コンビネーション Drive ノードを評価し、`mask_to_width` で駆動先信号の幅に切り詰めてから信号値を即時更新
     - 値が収束するまで（変更がなくなるまで）ループ
     - 1000回の反復で収束しなければ組合せループと判定してパニック
  3. `settled`取得: Phase 1収束後の全信号値をクローンする（regのクロックエッジ検出専用。クロック信号はモジュールの`clock`型ポートへ組み合わせDriveで接続されていることが多く、その組み合わせ網が収束した後の値でないと、このサイクルでのクロック変化を正しく検出できないため、代入値評価に使う`snapshot`（comb更新前）とは別に持つ）
  4. Phase 2 — 順序評価:
     - 全シーケンシャル Drive ノードについて、`should_update_reg`でこのサイクルに更新すべきか判定する。クロック紐付けの無いreg（モジュールの外のreg）は常に`true`
     - 更新すべきものだけ評価（参照する値は`snapshot`。Phase 1開始前の値、代入時のみ`mask_to_width`で幅に切り詰め）し、`next`配列に格納。更新しないものは`next`内の既存値（前サイクルの値）のまま据え置かれる
     - `next` → `signal_values` に一斉コミット、`settled` → `prev_signal_values`（次回`step`呼び出し時の比較用）に保存
  5. サイクルカウンタを進め、`CycleSnapshot` を返す

`Simulator::should_update_reg(&self, signal_id: usize, settled: &[u64]) -> bool` :

- 概要: このサイクルで`signal_id`のregを更新すべきか判定する。
- 処理: `self.signal_kinds[signal_id]`が`Reg { clock: Some(trigger), .. }`なら、`prev_signal_values[trigger.signal_id]`（1つ前のサイクルの収束後の値）と`settled[trigger.signal_id]`（このサイクルの収束後の値）を比較し、`trigger.edge`が`Posedge`なら`0→非0`、`Negedge`なら`非0→0`の遷移でのみ`true`。それ以外（`Wire`、または`clock`紐付けの無い`Reg`）は常に`true`（既存のステップ単位更新のまま）。

`Simulator::run(&mut self, nodes: &[Node], cycles: u64) -> Vec<CycleSnapshot>` :

- 概要: Nサイクル連続で実行し、全スナップショットを返す。
- 引数: `cycles` — 実行するサイクル数
- 返り値: `Vec<CycleSnapshot>` — サイクル0〜N-1 のスナップショット

`eval_and_mask(node_id: NodeId, nodes: &[Node], signal_values: &[u64], width: u64) -> u64` (公開関数) :

- 概要: ノードを評価し、指定した幅にマスクする。テストベンチの`initial`内`proc_assign`（`Simulator::step`を経由しない即時代入）が、通常の代入と同じマスキング規則で信号値を設定するために使う公開ラッパー。`main.rs`の`run_initial_sequence`から呼ばれる。
- 処理: `eval_node`と`mask_to_width`をそのまま呼ぶだけ。

`mask_to_width(value: u64, width: u64) -> u64` :

- 概要: 値を信号のビット幅に切り詰める（代入時のマスキング）。
- 処理: `width`が64以上ならそのまま返す（シフトオーバーフロー回避）。それ以外は `value & ((1 << width) - 1)` でビットマスクする。`Simulator::step`のPhase 1・Phase 2の両方で、Driveノードの評価結果に対して呼ばれる（`eval_and_mask`経由でも呼ばれる）。

`eval_node(node_id: NodeId, nodes: &[Node], signal_values: &[u64]) -> u64` :

- 概要: ノードID を指定して、そのノードの出力値を再帰的に計算する。
- 処理:
  - `Const` → 保持している定数値を返す
  - `ReadSignal` → `signal_values` から該当信号の値を返す
  - `BinOp` → 左右の子ノードを再帰評価し、`eval_binop` で演算子を適用する
  - `UnaryOp` → オペランドノードを再帰評価し、`eval_unaryop` で演算子を適用する
  - `Ternary` → `cond`/`then_branch`/`else_branch` を3つとも再帰評価し（ハードウェア的にはマルチプレクサなので選ばれない側も含めて常に評価する）、`cond`が0以外なら`then_branch`、0なら`else_branch`の値を返す
  - `Drive` → ソースノードを再帰評価して返す（値をそのまま中継）

`eval_binop(op: BinOp, l: u64, r: u64) -> u64` :

- 概要: 二項演算子を実際の `u64` 計算に適用する。
- 処理:
  - `Or`/`And` → 真偽値（`!= 0`）同士の論理演算、結果は0か1
  - `BitOr`/`Xor`/`BitAnd` → ビット単位の演算
  - `Eq`/`Neq`/`Lt`/`Le`/`Gt`/`Ge` → 比較結果を0か1で返す
  - `Shl`/`AShl`、`Shr`/`AShr` → `checked_shl`/`checked_shr`。シフト量が64以上ならNoneになるため0を返す（現状 `<<<`/`>>>` と `<<`/`>>` は同じ動作。この言語に符号付き型が無いため算術/論理シフトの区別を実装していない）
  - `Add`/`Sub`/`Mul` → `wrapping_*` でオーバーフローを丸める。式の途中経過（中間の`BinOp`）は信号の幅では切り詰められず、u64のラップアラウンドになる点に注意（信号への代入時にのみ`mask_to_width`で宣言幅に切り詰められる）
  - `Div`/`Mod` → `checked_div`/`checked_rem`。0除算は未定義値（'x'）が無いため0を返す

`eval_unaryop(op: UnOp, v: u64) -> u64` :

- 概要: 単項演算子を実際の `u64` 計算に適用する。
- 処理:
  - `Not` → 真偽値の否定（`v == 0`）、結果は0か1
  - `BitNot` → ビット単位の反転（`!v`）。`BinOp`と同様、ここでは幅マスキングを行わずu64の全ビット反転で近似し、信号への代入時にのみ`mask_to_width`で宣言幅に切り詰められる

`format_waveform(snapshots: &[CycleSnapshot], signals: &[NetlistSignal]) -> String` :

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

# CLI 使用方法

```bash
# サンプルコードを実行（ネットリスト表示まで）
cargo run

# ファイルを指定
cargo run -- path/to/file.tc

# シミュレーションまで実行（Nサイクル）
cargo run -- path/to/file.tc --cycles 10
cargo run -- --cycles 6   # サンプルコードを6サイクル
```

## オプション

| 引数                 | 説明                                  |
|----------------------|---------------------------------------|
| `--cycles N`, `-c N` | シミュレーションを N サイクル実行する（テストベンチに`initial`があれば無視される。下記参照） |

## Phase 4 の実行モード（`main.rs::run_simulation_phase`）

`main.rs`の4つのフェーズ関数（`run_parse_phase`/`run_elaboration_phase`/`run_netlist_phase`/`run_simulation_phase`）はいずれも「処理を実行する」と「結果を表示する」を分離しており、各フェーズの表示部分は対応する`print_*`関数（`print_parse_result`/`print_elaboration_result`/`print_netlist_result`/`print_simulation_result`）に切り出されている。Phase 4はさらに、実行そのものが分岐する（`initial`の有無）ため以下のようになっている:

- `nl.initial`が空でなければ`run_initial_sequence(nl) -> Vec<CycleSnapshot>`を呼ぶ（表示は一切行わない、純粋にスナップショット列を返すだけの関数）: `Simulator::new`で初期化し、`nl.initial`を順に処理する。`InitialStep::Assign { target, expr_node }`は`eval_and_mask`で値を求めて`Simulator::set_signal`、`InitialStep::Step`は`Simulator::step`を呼びスナップショットを記録する。得られたスナップショット列は`print_simulation_result("Testbench (initial)", &snaps, nl)`に渡して表示する。
- `nl.initial`が空なら、従来通り`--cycles`（`Some(n)`）が指定されていれば`Simulator::run`でNサイクル実行してスナップショット列を得て`print_simulation_result(&format!("Simulation ({n} cycles)"), &snaps, nl)`で表示する。指定が無ければ何もしない（Phase 3までのネットリスト表示で終わる）。
- `print_simulation_result(phase_label, snaps, nl)`はフェーズ見出し（`--- Phase 4: {phase_label} ---`）を表示したあと`format_waveform`で波形を表示する。フェーズラベルを引数化することで、`initial`実行と`--cycles`実行の見出し文言の違い（`Testbench (initial)` / `Simulation (N cycles)`）を1つの表示関数で吸収している。

---

# シミュレーションモデル

## 1サイクルの動作

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

## ノンブロッキング代入の動作

`b <= a` の例:

| Cycle | 開始時 a | Phase1 (comb) | Phase2 (seq: b) | 終了時 b |
|---|---|---|---|---|
| 0 | 0 | 変化なし | snapshot[a]=0 を評価 | 0 |
| 1 | 0 | a が comb で 1 に更新 | snapshot[a]=0 を評価 | 0 |
| 2 | 1 | 変化なし | snapshot[a]=1 を評価 | 1 |

→ b は a を1サイクル遅れで追従する。

---

# 拡張ポイント

現在のアーキテクチャで新しい機能を追加するときの変更箇所:

| 追加したい機能                        | 変更するファイル                                                                                                           |
|---------------------------------------|----------------------------------------------------------------------------------------------------------------------------|
| 単項マイナス（`-a`）                  | `ast.rs` (UnOp::Neg 追加), `grammar.pest` (unary_opに`-`追加), `parser.rs`, `netlist.rs`, `simulator.rs`                   |
| ビットベクタリテラルの桁区切り（`8'b0000_0001`） | `grammar.pest` (literal_digitsに`_`許容), `parser.rs` (パース時に`_`除去)                                    |
| クロックのnegedge/両エッジ対応（現状`Edge::Posedge`固定） | `grammar.pest`/`ast.rs`/`parser.rs`（ポート宣言でエッジ指定の構文を追加）, `elaboration.rs`（`clock_port`にエッジ情報を持たせる）, `netlist.rs`（`flatten_scope`のTODOコメント箇所で固定値をやめる）, `simulator.rs`（`should_update_reg`は`trigger.edge`を見るだけなので変更不要） |
| regのリセット対応（`SignalKind::Reg.reset`は先行実装のみで未使用） | `grammar.pest`/`ast.rs`/`parser.rs`（リセット信号・値を指定する構文を追加）, `elaboration.rs`, `netlist.rs`（`ResetSpec`を実際に埋める）, `simulator.rs`（`Simulator::step`でリセットトリガーを検出し値を上書き） |
| if/case 文                            | `ast.rs` (Stmt 拡張), `grammar.pest`, `parser.rs`, `netlist.rs` (Node 拡張), `simulator.rs`                                |
| モジュールのネスト（モジュールが別のモジュールをインスタンス化） | `grammar.pest` (`inst_decl`を`module_def`本体にも許可), `elaboration.rs` (`resolve_module_def`にインスタンス解決を追加、モジュール定義同士の循環インスタンス化を検出する依存グラフチェックを追加) |
| インスタンス境界をまたぐ組合せループの検出（エラボレーション時点） | `elaboration.rs` (モジュール定義ごとに`input`→`output`の組合せ依存を要約し、`InstanceField`の`collect_read_signals`をその要約で置き換える) |
| `assert`（テストベンチの値検証）      | `grammar.pest` (`initial_block`に`assert`文を追加), `ast.rs` (`ProcStmt::Assert`追加), `parser.rs`, `elaboration.rs` (`ResolvedProcStmt::Assert`), `netlist.rs` (`InitialStep::Assert`), `main.rs` (`run_initial_sequence`で評価しpass/fail報告) |
| VCD ダンプ                            | `simulator.rs` (format_waveform の代わりに VCD 出力)                                                                       |
