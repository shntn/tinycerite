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
program     = block+
block       = "{" (decl | stmt)* "}"
decl        = "var" ident ":" "bit" ("<" number ">")? ";"
stmt        = ident ("=" | "<=") expr ";"
expr        = 優先順位チェーン（|| > && > | > ^ > & > ==/!= > 比較 > シフト > +/- > */÷/%）
primary     = ident | number | "(" expr ")"
```

演算子の優先順位や各段の詳細は `docs/architecture.md` を参照。

- `var x: bit` — 1ビット信号を宣言（初期値 0）
- `var x: bit<N>` — Nビット信号を宣言
- `a = expr;` — 組み合わせ代入（即時反映）
- `a <= expr;` — 順序代入（サイクル開始時の値で評価、終了時に一斉反映）
- 演算子: `||` `&&` `|` `^` `&` `==` `!=` `<` `<=` `>` `>=` `<<` `>>` `<<<` `>>>` `+` `-` `*` `/` `%`（括弧`()`でグループ化可能）

### 制限（現在）

- リテラルは10進数のみ（ビットベクタ未対応）
- 単一ブロックのみ（モジュール・階層なし）
- ビット幅マスキング未実装（宣言幅を超える値もそのまま保持される）
- シミュレーションはパニックによる停止のみ（VCDダンプ未対応）

## アーキテクチャ

4段階パイプライン:

```
Source (.tc)
  ↓ Parser (pest) — 字句解析 + 構文解析
AST (Program)
  ↓ Elaboration — シンボル解決・型解決 + 多重ドライバ/組合せループ検出
Elaborated IR
  ↓ Netlist Builder — DAG構築
Netlist (信号DAG)
  ↓ Simulator — Δ-サイクル評価
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
