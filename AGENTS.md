# AGENTS.md

## 構成原則

分類は以下6領域に分け、MECE（漏れなく・重複なく）構成とする。

1. コミュニケーション
2. 品質保証
3. 実装方針
4. ドキュメント運用
5. ツール活用
6. 作業プロセス

---

## 1. コミュニケーション

- 応対言語は日本語のみ。全チャネルで統一する。
- 対話スタイルは簡潔かつ一貫性を保つ。曖昧な指示は禁止。
- 作業前に必ず `docs/` 配下の関連資料を確認する。

---

## 2. 品質保証

- タスク完了条件として `cargo test` 全通過を必須とする。
- テストをコメントアウトまたは無視する行為は禁止。
- 作業終了時には以下を実行する：
  - `cargo +nightly fmt && cargo build && cargo test`
- カバレッジ計測が必要な場合は `cargo make coverage` または `./coverage.sh` を使用する。

---

## 3. 実装方針

- 既存実装を参照し、一貫したスタイル・命名規約を保つ。
- 参考ベース：`docs/sources/protoactor-go`
  → protoactor-go の実装を Rust イディオムに置き換える。
- 後方互換性は不要。最適な設計を優先する。
- ドキュメント作成より実装を優先。対象ディレクトリ外の参照を禁止。
- ログ出力は `tracing` の `debug` レベルを基本とし、`info` は最小限。
- std依存は `#[cfg(feature = "std")]` でガードし、executor非依存のFutureを返す。

---

## 4. ドキュメント運用

- 文書構成は常に MECE を意識し、節や箇条書きの分割基準を明示する。
- 終了済み・履歴タスクは削除し、現状と今後のタスクのみ記載。履歴は `git log` で追跡。
- 残タスク確認時は `docs/` 配下を走査し、優先順位順に整理する。
- 作業前に関連ドキュメントを必ず確認し、更新があれば反映する。

---

## 5. ツール活用

- `CLAUDE.md` を常時参照する。
- `serena MCP` を積極的に活用する。
- `.mcp.json` および `.serena/project.yml` の ignore 設定を適切に保ち、不要ファイルを送信しない。

---

## 6. 作業プロセス

- `.claude/commands/git-commit.md` に従い、作業途中でも小まめに `git commit` する。
- 作業完了時に `cargo build && cargo test` を実行し、結果を記録する。
- `cargo build --workspace` により全クレートを同期ビルドし、CIと整合させる。
- Lint・Format を日常ルーチンに組み込む：
  - `cargo clippy --workspace --all-targets -D warnings`
  - `cargo +nightly fmt`
- テストポリシー：
  - 標準テストと `#[tokio::test]` を併用。
  - テストファイルは `tests.rs` に統合し、`*_test.rs` は禁止。
  - テスト命名規約：`test_<対象>_<期待>`
  - 共有状態は `Arc` または `AsyncBarrier` で保護。

---

## コーディングスタイル

- ファイル・関数：`snake_case`
- 型：`PascalCase`
- 定数：`SCREAMING_SNAKE_CASE`
- モジュール構成：Rust 2018 方針を遵守。`mod.rs` 禁止。ディレクトリ名と一致させる。

---

## プロジェクト構成

- `core/` : アクターランタイム実装および `tests.rs` による検証。
- `remote/` : gRPC 通信および `.proto` 生成物を管理。
- `utils/`, `message-derive/` : 共通ヘルパー・マクロを提供。
- `docs/` : 設計メモ・優先タスクを保持し、作業前に確認する。

---

## セキュリティと設定

- ログレベルは `RUST_LOG=debug`。リモート調査時に使用。
- 機密値は `.env` のみに格納。
- `grcov` と `llvm-tools-preview` を `rustup` と `cargo install` で整備し、CI と整合。

---

## 要約チェックリスト

- 応対言語：日本語のみ
- テスト完了条件：`cargo test` 全通過
- 実装方針：protoactor-go を参考に Rust 最適化
- ドキュメント：MECE・履歴削除・現行タスクのみ
- ツール：CLAUDE.md・serena MCP 参照
- 手順：docs確認 → commit分割 → build/test/format実施
