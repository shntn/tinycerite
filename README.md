# tinycerilte

最小の HDL シミュレータ。信号宣言・組み合わせ/順序代入・二項演算子（論理/ビット/比較/シフト/算術）をサポートする独自言語のソースコードをパースし、ネットリストを生成、シミュレーションまで実行する。

## クイックスタート

```bash
cargo run -- example/example1.tc --cycles 6
```

出力:

```
--- Phase 4: Simulation (6 cycles) ---

cycle  a  b
-----------
    0  1  0
    1  1  1
    2  0  1
    3  0  0
    4  1  0
    5  1  1
```

## 言語

### サンプル

```
{
    var a: bit;
    var b: bit;

    a = b ^ 1;
    b <= a;
}
```

### 文法

```
program       = (module_def | testbench_def | block)+
module_def    = "module" ident "{" port_block (decl | stmt)* "}"
port_block    = "port" "{" port_decl* "}"
port_decl     = ident ":" ("input" | "output") signal_type ";"
signal_type   = "clock" | ("bit" ("<" number ">")?)

testbench_def = "testbench" ident "{" (decl | inst_decl | stmt)* initial_block? "}"
initial_block = "initial" "{" (proc_assign | proc_step)* "}"
proc_assign   = ident "=" ternary_expr ";"
proc_step     = "step" ";"

block       = "{" (decl | inst_decl | stmt)* "}"
decl        = "var" ident ":" signal_type ";"
inst_decl   = "var" ident "=" ident "(" (named_arg ("," named_arg)*)? ")" ";"
named_arg   = ident ":" ternary_expr

stmt        = ident ("=" | "<=") ternary_expr ";"
ternary_expr= expr ("?" ternary_expr ":" ternary_expr)?  # 三項演算子、右結合、優先度は最低
expr        = 優先順位チェーン（|| > && > | > ^ > & > ==/!= > 比較 > シフト > +/- > */÷/% > 単項）
primary     = field_access | ident | bitvec_literal | number | "(" ternary_expr ")"
field_access= ident "." ident   # instance.output_port
```

演算子の優先順位や各段の詳細は `docs/architecture.md` を参照。

- `var x: bit` — 1ビット信号を宣言（初期値 0）
- `var x: bit<N>` — Nビット信号を宣言
- `var x: clock` — クロック型の信号を宣言（常に1ビット、`<N>`は書けない）。**テストベンチ内でのみ**宣言できる（`block`・モジュール本体では宣言不可）。詳細は「クロック」節を参照
- `N'b...` `N'o...` `N'd...` `N'h...` — ビットベクタリテラル。幅`N`と基数（`b`=2進 `o`=8進 `d`=10進 `h`=16進）を明示する（例: `4'b1010`、`8'hFF`）。宣言幅に収まらない桁は代入と同様に幅へ切り詰められる（エラーにはならない）
- `a = expr;` — 組み合わせ代入（即時反映）
- `a <= expr;` — 順序代入（サイクル開始時の値で評価、終了時に一斉反映）
- 演算子: `||` `&&` `|` `^` `&` `==` `!=` `<` `<=` `>` `>=` `<<` `>>` `<<<` `>>>` `+` `-` `*` `/` `%`（括弧`()`でグループ化可能）
- 前置単項演算子: `!`（論理否定、結果は1ビット） `~`（ビット反転、結果はオペランドと同じ幅）。連続して書ける（例: `!!a`）
- 三項演算子: `cond ? then_branch : else_branch`（すべての演算子の中で最も優先度が低く、右結合）。`cond`が0以外なら`then_branch`、0なら`else_branch`を選択する。ハードウェア的にはマルチプレクサなので両分岐とも常に評価される
- 代入結果は代入先信号のビット幅にマスクされる（例: `bit<4>`に17を代入すると`17 & 0b1111 = 1`）。式の途中経過（中間の演算結果）は幅で切り詰められず、u64のラップアラウンドになる
- `// ...` — 行コメント（改行まで無視される）

### モジュール

```
module adder {
    port {
        a: input bit<8>;
        b: input bit<8>;
        sum: output bit<8>;
    }

    sum = a + b;
}

{
    var x: bit<8>;
    var y: bit<8>;
    var z: bit<8>;

    var u1 = adder(a: x, b: y);   // 入力ポートだけを名前付き引数で接続
    z = u1.sum;                    // 出力ポートはインスタンスから読み出す
}
```

- `module name { port { ... } ... }` — モジュール定義。`port { }` ブロックの直後に内部の`var`宣言・代入文を書く
- ポート宣言 `name: input/output bit<N>;` / `name: input clock;` — モジュールの入出力信号。`input`ポートへの内部代入はエラー。クロックポートについては「クロック」節を参照
- インスタンス化 `var inst = module_name(port: expr, ...);` — `input`ポートだけを名前付き引数で接続する（構造体リテラルではなく関数呼び出し風の構文）
- 出力の読み出し `inst.output_port` — 通常の式の中でそのまま使える（例: `z = inst.sum + 1;`）
- モジュール定義は宣言された時点で（インスタンス化の有無によらず）1回だけ検証される
- モジュールが別のモジュールをインスタンス化すること（ネスト）は現状未対応
- 既知の制限: あるインスタンスの出力を同じインスタンスの入力に戻すような、インスタンス境界をまたぐ組合せループはエラボレーション時点では検出できない（`InstanceField`読み出しは依存グラフ上では葉として扱われるため）。実際にそのような回路を書いた場合、シミュレーション実行時のΔ-サイクル上限（`MAX_COMB_ITERATIONS`）でパニックとして検出される

### クロック

```
module adder {
    port { clk: input clock; a: input bit<8>; b: input bit<8>; sum: output bit<8>; }
    sum <= a + b;   // 順序代入(reg) → clk型ポートが無いとエラボレーションエラー
}

testbench tb {
    var clk: clock;      // clock型のvar宣言はテストベンチ内でのみ許可
    clk <= !clk;

    var x: bit<8>;
    var y: bit<8>;
    var u1 = adder(clk: clk, a: x, b: y);   // clock型ポートにはclock型の信号しか接続できない
}
```

- `clk: input clock;` — モジュールのポートを`clock`型として宣言する。`output`に`clock`は使えない（エラー）
- `var clk: clock;` — クロック信号の宣言。**テストベンチ内でのみ**許可される（`block`・モジュール本体で書くとエラー）。値の生成方法は普通の代入文のまま（`clk <= !clk;`で毎ステップ反転させるか、カウンタと`&`を組み合わせて分周する。1回の`step`/1サイクルが最小の時間刻みなので、クロックの1周期は自然に2ステップになる）
- モジュール内で順序代入（`<=`、reg相当）を1つでも使う場合、そのモジュールに`clock`型の入力ポートが必須（無いとエラボレーションエラー）
- モジュールに`clock`型の入力ポートは高々1つ（2つ以上あるとエラー）
- インスタンス化時、`clock`型ポートには`clock`型の信号だけを接続できる（逆に`clock`型の信号を`bit`型ポートに接続するのもエラー）。型検査は接続部分のみで、通常の代入文の右辺の型までは検査しない
- regの`clock`はモジュールの`clock`入力ポートに紐付く。モジュールの外（トップレベル/テストベンチ直下）の順序代入はモジュールに属さないため、この制約もクロックへの紐付けも対象外（例えば`counter <= counter + 1;`はモジュール外なのでそのまま書ける）
- 現状トリガーの向きはposedge固定（暫定処置。negedge/両エッジのサポートは未実装）。また、実際にレジスタがクロックのエッジでのみ更新されるという評価モデルの変更自体はまだ実装しておらず、今回追加したのはクロックの紐付けの静的チェックのみ

### テストベンチ

```
module adder {
    port { clk: input clock; a: input bit<8>; b: input bit<8>; sum: output bit<8>; }
    sum <= a + b;
}

testbench tb {
    var counter: bit<8>;
    counter <= counter + 1;

    var clk: clock;
    clk <= counter & 1;      // クロック分周（既存の&演算子だけで書ける。新しい構文は不要）

    var x: bit<8>;
    var y: bit<8>;
    var z: bit<8>;
    var u1 = adder(clk: clk, a: x, b: y);

    initial {
        x = 3;
        y = 4;
        step;               // 1サイクル進める
        step;
        z = u1.sum;
    }
}
```

`testbench name { ... }` はプログラム中に高々1つ書ける、`module`と対になるトップレベル構文。中身は2つの部分からなる:

- **並行部分**（`decl`/`inst_decl`/`stmt`、`initial`より前）— 今までの`block`と全く同じ意味論（常時並行に動く回路の接続）。クロック信号もここで普通の代入文として作る（`clk <= !clk;`のようにサイクルごとにトグルさせるか、`counter`と`&`を組み合わせて分周する。1回の`step`が最小の時間刻みなので、クロックの1周期は自然に2ステップになる）
- **`initial { }`（手続き部分、省略可）** — 上から順に実行される。`target = expr;`（`proc_assign`）はその瞬間に一度だけ値を設定する（継続的な駆動ではない。`Simulator::set_signal`相当）。`step;`は明示的にシミュレーションを1サイクル進める（`Simulator::step`相当）

`testbench`に`initial`がある場合、CLIの`--cycles`は無視され、`initial`の手続きに従って実行される（`step;`の回数だけサイクルが進む）。`initial`が無ければ（並行部分だけのテストベンチ、または`testbench`自体が無い場合）今まで通り`--cycles N`でNサイクル実行する。

現状`assert`のような検証構文は無い（波形出力を見て手動で確認する）。

### 制限（現在）

- ビットベクタリテラルの桁区切り（`8'b0000_0001`のような`_`）は未対応
- モジュールのネスト（モジュールが別のモジュールをインスタンス化すること）は未対応
- インスタンス境界をまたぐ組合せループの検出はエラボレーション時点では未対応（シミュレーション実行時には検出される。上記参照）
- `initial`内に`assert`のような検証構文は無い
- シミュレーションはパニックによる停止のみ（VCDダンプ未対応）
- regの`clock`トリガーはposedge固定（暫定処置。negedge/両エッジは未対応）。また、regがクロックのエッジでのみ更新されるという評価モデル自体は未実装（クロックへの紐付けの静的チェックのみ実装済み）

## アーキテクチャ

4段階パイプライン:

```
Source (.tc)
  ↓ Parser (pest) — 字句解析 + 構文解析
AST (Program)
  ↓ Elaboration — モジュール定義の解決・シンボル解決・型解決 + 多重ドライバ/組合せループ検出
Elaborated IR（モジュール階層を保持）
  ↓ Netlist Builder — モジュール階層をフラットなDAGへ展開
Netlist (信号DAG、階層情報は名前空間プレフィックスのみ)
  ↓ Simulator — Δ-サイクル評価（testbenchのinitialがあればその手続きに従って駆動）
波形出力
```

詳細は `docs/architecture.md` を参照。

## CLI

```bash
cargo run                          # サンプルコード（ネットリスト表示）
cargo run -- file.tc               # ファイル指定
cargo run -- file.tc --cycles 10   # シミュレーション実行
cargo run -- --cycles 6            # サンプルコードを6サイクル
```

`file.tc`内に`initial`を持つ`testbench`があれば、`--cycles`は無視され`initial`の手続き（`step;`の回数）に従って実行される。

## テスト

```bash
cargo test
cargo clippy -- -D warnings
```

## プロジェクト構成

```
src/
  lib.rs           クレートエクスポート
  main.rs          CLI エントリポイント
  grammar.pest     PEG 文法定義
  ast.rs           AST 型定義
  parser.rs        構文解析
  elaboration.rs   シンボル解決・静的チェック
  netlist.rs       ネットリスト生成
  simulator.rs     シミュレーション実行
tests/             結合テスト
docs/
  architecture.md  アーキテクチャドキュメント
  call-tree.md     関数呼び出しツリー
example/
  example1.tc      サンプルコード
```

## ライセンス

MIT
