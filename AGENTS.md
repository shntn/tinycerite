# AGENTS.md

Tiny Cerite — Tiny HDL Simulator.

## Stack
- rust 1.97+ (edition 2024)
- Deployed on macOS and Linux

## Structure
- `src/`      — Rust コード
- `tests/`    — テストコード
- `docs`/`    — ドキュメント
- `target/`   — ビルド時に生成されるファイル

## Commands
- Dev: `[your dev command]`
- Build: `orb -m tinycerite cargo build`
- Test: `orb -m tinycerite cargo test`
- Test one file: `[single-test command]`
- Lint: `orb -m tinycerite cargo clippy`

## Verification
変更を加えた後はこの順序で実行し、エラーを修正してください
1. `orb -m tinycerite cargo check`  - タイプエラーを修正する 
2. `orb -m tinycerite cargo test`   - 失敗したテストを修正する
3. `orb -m tinycerite cargo clippy` - Lintエラーを修正する

## Conventions
- Code-Style: 関数は一つのことだけを行うべきです。「そして」という言葉を使って説明する必要がある場合は、関数を分割してください
- Code-Style: 変数にはその内容に基づいて名前を付け、関数にはその動作に基づいて名前を付ける。
- Code-Style: 名前を省略しないでください。`getUserProfile` を `getUsrProf` のように省略しないでください。簡潔さよりも明瞭さが重要です
- Code-Style: 不要なコードはコメントアウトではなく、削除してください。Gitが記憶しています
- Code-Style: エラーは明示的に処理してください。例外を無視したり、エラーの戻り値を無視したりしないでください
- Code-Style: コメントにはコードを読めばわかることを書かないでください。コードから読み取れない制約や設計意図などを書いてください
- Code-Style: コードはシンプルに保ち、過剰な設計は避けてください
- Testing: 変更を加えたら、変更した箇所を確認するテストを作成してください。
- Testing: 実装の詳細ではなく、動作を検証するテストを作成する
- Testing: 各テストには明確なアサーションが1つ必要です。検証する内容に基づいて名前を付けてください
- Testing: モジュールの内部関数ではなく、公開APIをテストしてください
- Testing: モックよりも実際の依存関係を優先してください
- Testing: すべてのバグ修正には、修正を適用しないと失敗する回帰テストを含める必要があります
- Testing: タスクを完了する前に、型チェック、テスト、リンティングを実行してください

## Don't
- 勝手に Git にコミットしないでください。コミットする前に確認してください
- 勝手ににコードを変更しないでください。コードを変更する前に確認してください

## Preferences
- 新規ファイルを作成するよりも、既存ファイルを編集することを優先してください 

## Workflow
- 物事がうまくいかなくなったら、立ち止まって計画を練り直しましょう。無理に押し進めてはいけません

## Style
- 小規模で、特定の機能に特化したものを好む 
- ネストされた条件式よりも早期リターンを使用する
