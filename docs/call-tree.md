# 関数呼び出しツリー

`main()` を起点とした、実行時の関数呼び出し関係。
`(再帰)` は自分自身（またはループを介した間接的な自己呼び出し）を含むことを示す。

```
main                                               — エントリポイント。CLI引数解析から4フェーズの実行までを呼び出す
├─ parse_args                                    — CLI引数(--cycles/-c、ファイルパス)を読み取る
├─ load_source                                   — ファイルを読み込む。指定がなければ組み込みサンプルを返す
├─ run_parse_phase                               — Phase 1: パースし、結果をダンプする
│  └─ Parser::parse_program                     — pestで構文解析し、ASTのProgramを返す
│     └─ parse_block                            — blockルールのペアをBlockに変換する
│        ├─ parse_decl                          — declルールのペアをDeclに変換する(キーワード名を拒否)
│        │  └─ is_keyword                      — 文字列がキーワード(var/bit)か判定する
│        └─ parse_stmt                          — stmtルールのペアをStmtに変換する(キーワード名を拒否)
│           ├─ is_keyword                       — 文字列がキーワード(var/bit)か判定する
│           └─ parse_expr                       — XOR連鎖を左結合のBinOp木に畳み込む
│              └─ parse_primary                 — identまたはnumberをExprに変換する(キーワードを拒否)
│                 └─ is_keyword                 — 文字列がキーワード(var/bit)か判定する
├─ run_elaboration_phase                         — Phase 2: エラボレーションし、結果をダンプする
│  └─ elaborate                                 — 宣言/文を解決し、静的チェックを適用する
│     ├─ build_signals                          — 宣言からシンボルテーブルと信号リストを構築する(重複宣言はエラー)
│     ├─ resolve_stmts                          — 代入文の変数名をシンボルIDに解決する(未宣言はエラー)
│     │  ├─ Stmt::target                       — 代入先の変数名を返す
│     │  ├─ Stmt::expr                         — 右辺の式への参照を返す
│     │  └─ resolve_expr (再帰)                — ASTの式を再帰的に解決済み式へ変換する
│     ├─ check_multiple_drivers                 — 同一信号への複数ドライバ(多重代入)を検出する
│     └─ check_combinational_loops              — 組合せ代入間の循環依存を検出する
│        ├─ collect_read_signals (再帰)         — 式が参照する信号IDを再帰的に集める
│        └─ dfs (再帰)                          — 白/灰/黒の色付けで依存グラフの循環をDFS探索する
├─ run_netlist_phase                             — Phase 3: ネットリストを構築し、テキスト表示する
│  ├─ build_netlist                             — エラボレーション結果からネットリストを生成する
│  │  ├─ NetlistBuilder::new                   — 空のビルダーを生成する
│  │  ├─ NetlistBuilder::build_expr (再帰)     — 解決済み式からノードを再帰的に構築する
│  │  │  ├─ NetlistBuilder::make_const        — 定数ノードを生成する
│  │  │  ├─ NetlistBuilder::make_read_signal  — 信号読み出しノードを生成する
│  │  │  └─ NetlistBuilder::make_binop        — 二項演算ノードを生成する
│  │  └─ NetlistBuilder::make_drive            — 信号駆動ノードを生成する
│  │     ├─ NetlistBuilder::alloc_id           — 新しいノードIDを割り当てる(make_const等も同様に呼ぶ)
│  │     └─ NetlistBuilder::add_node           — ノードをリストに追加しIDを返す(make_const等も同様に呼ぶ)
│  └─ format_netlist                            — ネットリストを読みやすいテキストに整形する
└─ run_simulation_phase                          — Phase 4: 指定サイクル数シミュレーションし、波形を表示する
   ├─ Simulator::new                             — 全信号を0で初期化する
   ├─ Simulator::run                             — Nサイクル連続実行し、スナップショット列を返す
   │  └─ Simulator::step                        — 1サイクル分評価し、結果のスナップショットを返す
   │     └─ eval_node (再帰)                    — ノードの出力値を再帰的に計算する
   └─ format_waveform                            — シミュレーション結果をテキスト波形に整形する
```

## 補足

- `NetlistBuilder::make_const` / `make_read_signal` / `make_binop` も `make_drive` と同様に内部で
  `alloc_id` → `add_node` の順に呼ぶが、重複を避けるため図では `make_drive` の下にのみ展開している。
- `parse_expr` は `primary ("^" primary)*` の各項について `parse_primary` をループ内で呼ぶが、
  呼び出し先は同じなので1本の枝にまとめている。
