# 関数呼び出しツリー

`main()` を起点とした、実行時の関数呼び出し関係。`(再帰)` は自分自身（またはループを介した間接的な自己呼び出し）を含むことを示す。

```
main                                                 — エントリポイント。CLI引数解析から4フェーズの実行までを呼び出す
├─ parse_args                                        — CLI引数(--cycles/-c、ファイルパス)を読み取る
├─ load_source                                       — ファイルを読み込む。指定がなければ組み込みサンプルを返す
├─ run_parse_phase                                   — Phase 1: パースし、結果をダンプする
│  └─ Parser::parse_program                          — pestで構文解析し、ASTのProgramを返す
│     └─ parse_block                                 — blockルールのペアをBlockに変換する
│        ├─ parse_decl                               — declルールのペアをDeclに変換する(キーワード名を拒否)
│        │  └─ is_keyword                            — 文字列がキーワード(var/bit)か判定する
│        └─ parse_stmt                               — stmtルールのペアをStmtに変換する(キーワード名を拒否)
│           ├─ is_keyword                            — 文字列がキーワード(var/bit)か判定する
│           └─ parse_ternary_expr                    — 三項演算子(cond ? then : else)を解決する。式の最上位はここから入る(下記補足)
│              ├─ parse_expression                   — 優先順位チェーンの最上段(||)。9段連鎖の入口(下記補足)
│              │  └─ parse_expression_unary          — 前置単項演算子(!/~)の連鎖を解決する
│              │     └─ parse_expression_factor      — 連鎖の最下段。ident/number/括弧を解決する
│              │        ├─ is_keyword                — 文字列がキーワード(var/bit)か判定する
│              │        └─ parse_ternary_expr (再帰) — 括弧の中身を再帰的に解決する
│              └─ parse_ternary_expr (再帰)          — then/else分岐を右結合で再帰的に解決する
├─ run_elaboration_phase                             — Phase 2: エラボレーションし、結果をダンプする
│  └─ elaborate                                      — 宣言/文を解決し、静的チェックを適用する
│     ├─ build_signals                               — 宣言からシンボルテーブルと信号リストを構築する(重複宣言はエラー)
│     ├─ resolve_stmts                               — 代入文の変数名をシンボルIDに解決する(未宣言はエラー)
│     │  ├─ Stmt::target                             — 代入先の変数名を返す
│     │  ├─ Stmt::expr                               — 右辺の式への参照を返す
│     │  └─ resolve_expr (再帰)                      — ASTの式を再帰的に解決済み式へ変換する
│     ├─ check_multiple_drivers                      — 同一信号への複数ドライバ(多重代入)を検出する
│     └─ check_combinational_loops                   — 組合せ代入間の循環依存を検出する
│        ├─ build_combinational_deps                 — 組合せ代入の依存グラフを構築する
│        │  └─ collect_read_signals (再帰)           — 式が参照する信号IDを再帰的に集める
│        └─ dfs_visit (再帰)                         — 依存グラフをDFSで訪問し、循環を検出する
│           └─ cycle_error                           — 循環発見時、経路を含むエラーメッセージを組み立てる
├─ run_netlist_phase                                 — Phase 3: ネットリストを構築し、テキスト表示する
│  ├─ build_netlist                                  — エラボレーション結果からネットリストを生成する(Driveに駆動先の幅も渡す)
│  │  ├─ NetlistBuilder::new                         — 空のビルダーを生成する
│  │  ├─ NetlistBuilder::build_expr (再帰)           — 解決済み式からノードを再帰的に構築する(BinOp/UnaryOp/Ternaryの結果幅も決定)
│  │  │  ├─ NetlistBuilder::make_const               — 定数ノードを生成する
│  │  │  ├─ NetlistBuilder::make_read_signal         — 信号読み出しノードを生成する
│  │  │  ├─ NetlistBuilder::make_binop               — 二項演算ノードを生成する
│  │  │  ├─ NetlistBuilder::make_unaryop             — 単項演算ノードを生成する
│  │  │  └─ NetlistBuilder::make_ternary             — 三項演算ノードを生成する
│  │  └─ NetlistBuilder::make_drive                  — 信号駆動ノードを生成する(駆動先信号の幅を保持)
│  │     ├─ NetlistBuilder::alloc_id                 — 新しいノードIDを割り当てる(make_const等も同様に呼ぶ)
│  │     └─ NetlistBuilder::add_node                 — ノードをリストに追加しIDを返す(make_const等も同様に呼ぶ)
│  └─ format_netlist                                 — ネットリストを読みやすいテキストに整形する
└─ run_simulation_phase                              — Phase 4: 指定サイクル数シミュレーションし、波形を表示する
   ├─ Simulator::new                                 — 全信号を0で初期化する
   ├─ Simulator::run                                 — Nサイクル連続実行し、スナップショット列を返す
   │  └─ Simulator::step                             — 1サイクル分評価し、結果のスナップショットを返す
   │     ├─ eval_node (再帰)                         — ノードの出力値を再帰的に計算する(TernaryはcondとThen/Elseを3つとも再帰評価してから選択)
   │     │  ├─ eval_binop                            — 二項演算子をu64の実計算に適用する
   │     │  └─ eval_unaryop                          — 単項演算子をu64の実計算に適用する
   │     └─ mask_to_width                            — 代入結果を信号のビット幅に切り詰める(代入時のマスキング)
   └─ format_waveform                                — シミュレーション結果をテキスト波形に整形する
```

## 補足

- `NetlistBuilder::make_const` / `make_read_signal` / `make_binop` / `make_unaryop` / `make_ternary` も `make_drive` と同様に内部で `alloc_id` → `add_node` の順に呼ぶが、重複を避けるため図では `make_drive` の下にのみ展開している。
- `parse_expression` は実際には `parse_expression1`〜`parse_expression9` という9段の優先順位チェーン（`||` → `&&` → `|` → `^` → `&` → `==`/`!=` → 比較 → シフト → `+`/`-` → `*`/`/`/`%`）になっており、各段は共通ヘルパー `parse_left_assoc` を介して1つ下の段を呼ぶ。すべて構造が同じ（左結合の`Expr::BinOp`木を組み立てるだけ）なので、中間の8段は省略し、最上段(`parse_expression`)と最下段(`parse_expression9`が呼ぶ`parse_expression_unary`〜`parse_expression_factor`)だけを示している。詳細は `docs/architecture.md` の `parser` モジュール節を参照。
- `parse_expression_unary` は `!`/`~` の連鎖を集めて `parse_expression_factor` のオペランドに右結合で被せる（`!~a` → `Not(BitNot(a))`）。単項演算子は乗除算より優先度が高いため、`expression9`(`parse_expression9`) と `expression_factor` の間に位置する。
- `parse_ternary_expr` は二項演算子チェーン全体（`parse_expression`）よりさらに外側にあり、`stmt` の右辺・括弧の中身のどちらもここから式の解決に入る。`?`/`:` が続かなければ条件部（`parse_expression`が返した式）をそのまま返し、続く場合は then/else をそれぞれ自分自身に再帰させることで `a ? b : c ? d : e` が `a ? b : (c ? d : e)`（右結合）になる。
- `mask_to_width` は `eval_node`/`eval_binop`/`eval_unaryop` の計算結果に対して、`Simulator::step` が代入の瞬間にだけ適用する。式の途中経過（中間の`BinOp`/`UnaryOp`/`Ternary`評価）はマスクされない。
