# 関数呼び出しツリー

`main()` を起点とした、実行時の関数呼び出し関係。`(再帰)` は自分自身（またはループを介した間接的な自己呼び出し）を含むことを示す。

```
main                                                 — エントリポイント。CLI引数解析から4フェーズの実行までを呼び出す
├─ parse_args                                        — CLI引数(--cycles/-c、ファイルパス)を読み取る
├─ load_source                                       — ファイルを読み込む。指定がなければ組み込みサンプルを返す
├─ run_parse_phase                                   — Phase 1: パースし、結果をダンプする
│  └─ Parser::parse_program                          — pestで構文解析し、ASTのProgramを返す
│     ├─ parse_module_def                            — moduleルールのペアをModuleDefに変換する(キーワード名を拒否)
│     │  ├─ is_keyword                               — 文字列がキーワード(var/bit/module/port/input/output/testbench/initial/step)か判定する
│     │  ├─ parse_port_block                         — port_blockルールのペアをVec<PortDecl>に変換する
│     │  │  └─ parse_port_decl                       — port_declルールのペアをPortDeclに変換する(キーワード名を拒否)
│     │  │     └─ is_keyword                         — 文字列がキーワードか判定する
│     │  ├─ parse_decl                               — declルールのペアをDeclに変換する(キーワード名を拒否)
│     │  │  └─ is_keyword                            — 文字列がキーワードか判定する
│     │  └─ parse_stmt                               — block内のparse_stmtと同一(下記に展開)
│     ├─ parse_testbench_def                         — testbenchルールのペアをTestbenchDefに変換する(キーワード名を拒否)
│     │  ├─ is_keyword                               — 文字列がキーワードか判定する
│     │  ├─ parse_decl                               — block内のparse_declと同一(上記に展開)
│     │  ├─ parse_inst_decl                          — block内のparse_inst_declと同一(下記に展開)
│     │  ├─ parse_stmt                               — block内のparse_stmtと同一(下記に展開)
│     │  └─ parse_initial_block                      — initial_blockルールのペアをVec<ProcStmt>に変換する
│     │     └─ parse_proc_assign                     — proc_assignルールのペアをProcStmt::Assignに変換する(キーワード名を拒否)
│     │        ├─ is_keyword                         — 文字列がキーワードか判定する
│     │        └─ parse_ternary_expr                 — block内のparse_stmtが呼ぶものと同一の式解決チェーンへ(下記に展開)
│     └─ parse_block                                 — blockルールのペアをBlockに変換する
│        ├─ parse_decl                               — declルールのペアをDeclに変換する(キーワード名を拒否)
│        │  └─ is_keyword                            — 文字列がキーワードか判定する
│        ├─ parse_inst_decl                          — inst_declルールのペアをInstDeclに変換する(キーワード名を拒否)
│        │  ├─ is_keyword                            — 文字列がキーワードか判定する
│        │  └─ parse_named_arg                       — named_argルールのペアを(ポート名, 式)に変換する
│        │     └─ parse_ternary_expr                 — 下記のparse_stmt内と同一の式解決チェーンへ(下記に展開)
│        └─ parse_stmt                               — stmtルールのペアをStmtに変換する(キーワード名を拒否)
│           ├─ is_keyword                            — 文字列がキーワードか判定する
│           └─ parse_ternary_expr                    — 三項演算子(cond ? then : else)を解決する。式の最上位はここから入る(下記補足)
│              ├─ parse_expression                   — 優先順位チェーンの最上段(||)。9段連鎖の入口(下記補足)
│              │  └─ parse_expression_unary          — 前置単項演算子(!/~)の連鎖を解決する
│              │     └─ parse_expression_factor      — 連鎖の最下段。field_access/ident/number/bitvec_literal/括弧を解決する
│              │        ├─ is_keyword                — 文字列がキーワードか判定する
│              │        ├─ parse_bitvec_literal      — ビットベクタリテラル(4'b1010等)を幅・基数付きで解決する
│              │        ├─ parse_field_access        — field_access(instance.field)をExpr::FieldAccessに変換する
│              │        │  └─ is_keyword             — 文字列がキーワードか判定する
│              │        └─ parse_ternary_expr (再帰) — 括弧の中身を再帰的に解決する
│              └─ parse_ternary_expr (再帰)          — then/else分岐を右結合で再帰的に解決する
├─ run_elaboration_phase                             — Phase 2: エラボレーションし、結果をダンプする
│  └─ elaborate                                      — モジュール定義を解決し、トップレベルを解決したあと、initialを解決する
│     ├─ build_module_defs                           — 全モジュール定義を1回ずつ解決する(重複定義はエラー)
│     │  └─ resolve_module_def                       — モジュール定義を解決・検証する(宣言時点で1回だけ)
│     │     ├─ resolve_module_ports                  — ポート宣言を信号として登録する(重複ポート名はエラー)
│     │     ├─ resolve_module_decls                  — 内部のvar宣言をポートと同じ信号空間に追加登録する(重複はエラー)
│     │     ├─ resolve_module_stmts                  — 本体の代入文を解決する(inputポートへの代入はエラー)
│     │     │  └─ resolve_expr (再帰)                — ASTの式を再帰的に解決済み式へ変換する(InstanceFieldの解決も含む)
│     │     ├─ check_multiple_drivers                — 同一信号への複数ドライバ(多重代入)を検出する
│     │     └─ check_combinational_loops             — 組合せ代入間の循環依存を検出する
│     │        ├─ build_combinational_deps           — 組合せ代入の依存グラフを構築する
│     │        │  └─ collect_read_signals (再帰)     — 式が参照する信号IDを再帰的に集める(InstanceFieldは葉として扱う)
│     │        └─ dfs_visit (再帰)                   — 依存グラフをDFSで訪問し、循環を検出する
│     │           └─ cycle_error                     — 循環発見時、経路を含むエラーメッセージを組み立てる
│     ├─ elaborate_top                               — トップレベルのブロック群(とテストベンチの並行部分)を解決する
│     │  ├─ build_signals                            — 宣言からシンボルテーブルと信号リストを構築する(重複宣言はエラー)
│     │  ├─ build_instances                          — モジュールインスタンス化を解決する(引数をポート定義と突き合わせる)
│     │  │  └─ resolve_expr (再帰)                   — 接続式を解決する(上記と同一関数)
│     │  ├─ resolve_stmts                            — 代入文の変数名をシンボルIDに解決する(未宣言はエラー)
│     │  │  ├─ Stmt::target                          — 代入先の変数名を返す
│     │  │  ├─ Stmt::expr                            — 右辺の式への参照を返す
│     │  │  └─ resolve_expr (再帰)                   — ASTの式を再帰的に解決済み式へ変換する(上記と同一関数)
│     │  ├─ check_multiple_drivers                   — 上記と同一関数
│     │  └─ check_combinational_loops                — 上記と同一関数(展開は上記を参照)
│     └─ resolve_initial                             — テストベンチのinitial(手続き文)を解決する
│        └─ resolve_expr (再帰)                      — proc_assignの右辺を解決する(上記と同一関数)
├─ run_netlist_phase                                 — Phase 3: ネットリストを構築し、テキスト表示する
│  ├─ build_netlist                                  — エラボレーション結果からネットリストを生成する
│  │  ├─ NetlistBuilder::flatten_scope (再帰)        — スコープをフラットな信号・ノードへ再帰的に展開する(下記補足)
│  │  │  ├─ scoped_name                              — 信号名に名前空間プレフィックスを付ける(例: "u1.sum")
│  │  │  ├─ NetlistBuilder::build_expr (再帰)        — 解決済み式からノードを再帰的に構築する(BinOp/UnaryOp/Ternaryの結果幅も決定)
│  │  │  │  ├─ NetlistBuilder::make_const            — 定数ノードを生成する(Number/BitVecLiteralが共用)
│  │  │  │  ├─ NetlistBuilder::make_read_signal      — 信号読み出しノードを生成する(Ident/InstanceFieldが共用)
│  │  │  │  ├─ NetlistBuilder::make_binop            — 二項演算ノードを生成する
│  │  │  │  ├─ NetlistBuilder::make_unaryop          — 単項演算ノードを生成する
│  │  │  │  └─ NetlistBuilder::make_ternary          — 三項演算ノードを生成する
│  │  │  └─ NetlistBuilder::drive_signal             — 信号をDriveノードで駆動し、駆動情報を更新する共通ヘルパー
│  │  │     └─ NetlistBuilder::make_drive            — 信号駆動ノードを生成する(駆動先信号の幅を保持)
│  │  │        ├─ NetlistBuilder::alloc_id           — 新しいノードIDを割り当てる(make_const等も同様に呼ぶ)
│  │  │        └─ NetlistBuilder::add_node           — ノードをリストに追加しIDを返す(make_const等も同様に呼ぶ)
│  │  └─ NetlistBuilder::build_expr (再帰)           — initialのAssign式もここで構築する(上記と同一関数)
│  └─ format_netlist                                 — ネットリストを読みやすいテキストに整形する
└─ run_simulation_phase                              — Phase 4: initialがあればその手続きを、無ければ--cyclesでNサイクル実行する
   ├─ run_initial_sequence                           — initialが空でない場合、AssignはSimulator::set_signalで即時反映し、StepでSimulator::stepを呼ぶ
   │  ├─ Simulator::new                              — 全信号を0で初期化する
   │  ├─ eval_and_mask                               — ノードを評価し指定幅にマスクする(Assignの値計算に使う)
   │  │  ├─ eval_node (再帰)                         — 下記のSimulator::step内と同一関数(下記に展開)
   │  │  └─ mask_to_width                            — 下記のSimulator::step内と同一関数(下記に展開)
   │  ├─ Simulator::set_signal                       — 信号に値を即時設定する
   │  ├─ Simulator::step                             — 1サイクル分評価し、結果のスナップショットを返す(下記に展開)
   │  └─ format_waveform                             — 記録したスナップショット列をテキスト波形に整形する(下記と同一関数)
   ├─ Simulator::new                                 — initialが無い場合: 全信号を0で初期化する
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
- `parse_ternary_expr` は二項演算子チェーン全体（`parse_expression`）よりさらに外側にあり、`stmt` の右辺・括弧の中身のどちらもここから式の解決に入る。`?`/`:` が続かなければ条件部（`parse_expression`が返した式）をそのまま返し、続く場合は then/else をそれぞれ自分自身に再帰させることで `a ? b : c ? d : e` が `a ? b : (c ? d : e)`（右結合）になる。`parse_module_def`内の`parse_stmt`・`parse_testbench_def`内の`parse_stmt`/`parse_proc_assign`・`parse_inst_decl`内の`parse_named_arg`が呼ぶ`parse_ternary_expr`は、`parse_block`内の`parse_stmt`が呼ぶものと完全に同じ関数・同じ式解決チェーンなので、図では1箇所だけ展開している。
- `parse_bitvec_literal` は `4'b1010`・`8'hFF` のような幅付きリテラルを解決する。基数に合わない桁（例: `2'b19`）は `u64::from_str_radix` のエラーをそのまま`ParseError`として返す。宣言幅への切り詰めはここでは行わず、`NetlistBuilder::build_expr` で`make_const`に渡す直前にまとめて行う。
- `parse_testbench_def`内の`parse_decl`/`parse_inst_decl`/`parse_stmt`は、`parse_block`内で呼ばれるものと全く同じ関数（テストベンチの並行部分は`block`と同じ意味論のため）。図では重複を避けて`parse_block`側だけ展開している。
- `elaborate` はモジュール定義の解決（`build_module_defs`）→トップレベル解決（`elaborate_top`）→テストベンチの`initial`解決（`resolve_initial`）の順に進む。`resolve_module_def`（内部で`resolve_module_ports`→`resolve_module_decls`→`resolve_module_stmts`の3段に分割されている）と`elaborate_top`はどちらも「信号を解決 → 代入文を解決（`resolve_expr`）→ `check_multiple_drivers`/`check_combinational_loops`を適用」という同じ形をしており、`check_*`系の関数は完全に同じものを呼んでいるため、図では`resolve_module_def`側だけ展開し、`elaborate_top`側は「上記と同一関数」と注記している。`elaborate_top`は`SymbolTable`/`InstanceTable`も返し、`resolve_initial`がそれを再利用する。
- `resolve_expr`は4箇所（モジュール本体の代入文、インスタンス化の接続式、トップレベルの代入文、テストベンチの`initial`内`proc_assign`の右辺）から呼ばれるが、すべて同じ関数。モジュール本体を解決する際は空の`InstanceTable`を渡すため、`Expr::FieldAccess`が現れても「インスタンスが見つからない」エラーに自然に倒れる（現状モジュール本体はインスタンスを持てないため、この経路は文法上そもそも通らない）。
- `check_combinational_loops`の`collect_read_signals`は、`ResolvedExpr::InstanceField`（`u1.sum`のような読み出し）を依存なしの葉として扱う。そのため、インスタンス境界をまたぐ組合せループ（あるインスタンスの出力を同じインスタンスの入力に戻すような配線）はエラボレーション時点では検出できず、ネットリスト構築後にシミュレータのΔ-サイクル上限（`mask_to_width`の手前、`Simulator::step`の組合せ評価ループ）で検出されることになる。詳細は `docs/architecture.md` の `elaboration` モジュール節を参照。
- `NetlistBuilder::flatten_scope` はモジュール階層を再帰的に辿ってフラットな`signals`/`nodes`へ展開する。トップレベルスコープに対して1回呼ばれ、`scope.instances`があればそのモジュール本体を（インスタンス名をプレフィックスにして）再帰的に自分自身へ渡す。戻り値の`remap`/`instance_remaps`は、その直後に`build_netlist`が`initial`のAssign式を構築する際にも再利用される。展開が終わった時点で`Node`/`NetlistSignal`はモジュールの存在を一切知らないため、`Simulator::step`以下はモジュール対応前と全く同じまま変更されていない。
- `run_initial_sequence`は`nl.initial`が空でない場合にのみ呼ばれ、空なら従来通り`Simulator::run`（`--cycles`分だけ`Simulator::step`を繰り返す）が使われる。両者は排他的で、テストベンチに`initial`があれば`--cycles`は無視される。
- `eval_and_mask`は`eval_node`と`mask_to_width`を呼ぶだけの公開ラッパーで、`Simulator::step`を経由しない`run_initial_sequence`（`proc_assign`の値計算）のために用意されている。
- `mask_to_width` は `eval_node`/`eval_binop`/`eval_unaryop` の計算結果に対して、`Simulator::step`（および`eval_and_mask`経由で`run_initial_sequence`）が代入の瞬間にだけ適用する。式の途中経過（中間の`BinOp`/`UnaryOp`/`Ternary`評価）はマスクされない。
