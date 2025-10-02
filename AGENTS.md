# Repository Guidelines

## 重要な注意事項

分割基準: 以下の項目はコミュニケーション、品質保証、実装方針、ドキュメント運用、ツール活用、作業プロセスの6領域で漏れ・重複なく整理しています。

### コミュニケーション

- 応対言語は常に日本語とし、チャネルを問わず統一します。

### 品質保証

- タスク完了条件として `cargo test` 系を含む全テストのパスを必須とします。
- 実行すべきテストをコメントアウト・無視しないこと。

### 実装方針

- 既存実装を参照し一貫性のあるコードを書きます。
- `docs/sources/protoactor-go` を手がかりに protoactor-go の実装を Rust イディオムへ変換して活用します。
- 後方互換性は要求しないため、最適な設計を優先します。
- ドキュメント作成より実装を優先し、当該ディレクトリ外のファイルは参照しません。

### ドキュメント運用

- 文章構成は常に MECE を意識し、節や箇条書きの分割基準を明記します。
- 終了済みタスクや履歴は削除し、現状と今後のタスクのみ簡潔に記載します（履歴は `git log` を参照）。
- 作業前には docs 配下の関連ドキュメントを必ず確認します。
- 残タスク確認時は docs 配下を調査し、優先順位順に整理して提示します。

### ツール活用

- `CLAUDE.md` を常に参照します。
- serena MCP を積極的に活用します。

### 作業プロセス

- 作業途中でも `.claude/commands/git-commit.md` に従い適宜 git commit を行います。
- 作業終了時には `cargo +nightly fmt && cargo build && cargo test` を実行し結果を確認します。

## プロジェクト構成とモジュール

- core ディレクトリ で アクター ランタイム を 実装 し 末尾 tests.rs で 検証。
- remote では gRPC ベース 通信 と modules/proto の .proto 生成 物 を 管理。
- utils と message-derive が 共通 ヘルパー と マクロ を 提供 し 他 クレート から 再利用。
- docs フォルダ で 設計 メモ や 優先 タスク を 参照 し 作業 前 に 更新 します。

## ビルド・テスト・開発コマンド

- `cargo build --workspace` で 全 クレート を 同期 ビルド し CI と 揃えます。
- `cargo test --workspace` が 既定、ボトルネック 調査 は `cargo test -p <crate>` や `-- --nocapture`。
- `cargo clippy --workspace --all-targets -D warnings` と `cargo +nightly fmt` を 日常 サイクル に 組み込みます。
- カバレッジ が 必要 な 際 は `cargo make coverage` または `./coverage.sh` を 実行 します。

## コーディングスタイルと命名規約

- snake_case ファイル と 関数、PascalCase 型、SCREAMING_SNAKE_CASE 定数 を 徹底 します。
- Rust 2018 モジュール 方針 を 守り mod.rs を 禁止、ディレクトリ と ファイル 名 を 一致 させます。
- std 依存 は `#[cfg(feature = "std")]` で ガード し 非同期 API は executor 非依存 の Future を 返却。
- ログ は `tracing` の debug レベル を 基本 と し 過剰 な info 出力 を 控えます。

## テスト指針

- Rust 標準 と `#[tokio::test]` を 併用 し 正常 系 と エラー 系 シナリオ を カバー。
- テスト ファイル は 実装 と 同階層 の tests.rs に まとめ `*_test.rs` は 使用 しません。
- テスト 名 は `test_<対象>_<期待>` 形式 と し 共有 状態 は `Arc` 互換 抽象 や `AsyncBarrier` で 保護。
- 重要 機能 は `cargo make coverage` など で カバレッジ を 把握 し 回帰 を 防ぎます。

## コミット と Pull Request ガイドライン

- コミット メッセージ は `<type>: <要約>` を 原則 と し 例 は `refactor: clean up dispatcher tests`。
- PR には 背景、変更 点、実行 済み コマンド (`cargo test` `cargo clippy`) を 箇条書き し Issue を 紐付け。
- 挙動 変更 時 は 再現 手順 や ログ を 添付 し docs/ 配下 の 優先 タスク も 更新 します。
- 作業 終了 時 は `cargo build && cargo test` で 最終 検証 を 行い 結果 を 記録。

## セキュリティ と 設定

- `RUST_LOG=debug` で リモート 調査 を 行い 機密 値 は `.env` に 限定 します。
- `coverage.sh` 用 の `grcov` と `llvm-tools-preview` を rustup と cargo install で 揃え CI との差異 を 防止。
- `.mcp.json` と `.serena/project.yml` の ignore 設定 を 換装 し 不要 な ファイル を MPC に 送らない よう 管理。
