# Repository Guidelines

## プロジェクト構成とモジュール
本リポジトリ は Cargo ワークスペース として構成され、主要クレート を 明確 に 分離 しています。`core/` は アクター ランタイム と メッセージング の中核、`cluster/` は メンバーシップ と ゴシップ の拡張、`remote/` は RPC ベース の リモート 通信、`message-derive/` は マクロ 実装、`utils/` は サポート ユーティリティ を 提供 します。テスト コード は 各クレート の `src/**/tests.rs` に 隣接 配置 され、ユニット と 統合 の 区分 を 明確 に 保っています。共通 設定 は ルート の `Cargo.toml` や `rust-toolchain.toml` で 一元 管理 されるため、変更 時 は ワークスペース 全体 への 影響 を 確認 してください。

## ビルド・テスト・開発コマンド
- `cargo build --workspace` : すべてのクレート を ビルド し、依存 関係 変更 の 破壊 を 早期 検知 します。
- `cargo test --workspace` : 全テスト を 並行 実行。個別 クレート は `cargo test -p core actor::dispatcher::tests::` のように プレフィックス 指定 が 可能 です。
- `cargo clippy --workspace --all-targets` : Lint を 通じて API 変更 由来 の 回 regressions を 事前 捕捉。
- `cargo +nightly fmt` : ワークスペース 共通 の `rustfmt.toml` 設定 (幅 120、2 スペース インデント) を 適用 します。
- `cargo make coverage` または `./coverage.sh` : grcov を 用いた HTML レポート を `target/coverage/html/index.html` に 生成。

## コーディングスタイルと命名規約
Rust 標準 に 従い、モジュール と ファイル は snake_case、型 と トレイト は PascalCase、定数 は SCREAMING_SNAKE_CASE を 採用 します。非同期 ロジック では `tracing` を 用いた ログ を 最小 限度 で 残し、`?` に よる 早期 return と `thiserror` の エラー ラップ を 優先 してください。PR は フォーマット 済み (`cargo +nightly fmt` 実行 済み) かつ Lint 無し (`cargo clippy` の警告 0) の 状態 を 必須 と します。

## テストガイドライン
テスト は モジュール 内部 の `tests` サブモジュール に 追加 し、対象 機能 名 を 先頭 に 付けた snake_case で 命名 (`test_future_pipe_to_message` など) してください。非同期 テスト では `#[tokio::test]` を 基本 と し、共有 状態 には `Arc` や `AsyncBarrier` を 利用 して 競合 を 排除 します。新規 機能 では エラーパス と 正常 系 の 双方 を カバー し、カバレッジ 実行 結果 を PR に 添付 する こと が 推奨 です。

## コミットと Pull Request ガイドライン
コミット メッセージ は `refactor: tidy up...` のように 小文字 タイプ + コロン + 簡潔 な 説明 で 統一 してください (`feat`、`fix`、`refactor` など)。PR では 変更 背景、テスト 実行 結果 (`cargo test` と `cargo clippy`)、関連 Issue の リンク を 箇条書き で 記載。振る舞い 変更 が 可視 的 な 場合 は ログ や スクリーンショット を 添付 し Reviewer が 再現 できる よう に します。

## 追加リソース
詳細 な 状況 や 長期 ロードマップ は `PROJECT_STATUS.md` を、補助 ドキュメント と 自動化 ポリシー は `CLAUDE.md` を 参照 してください。新しい ガイドライン を 追加 する 場合 は 本書 と 併せ て 同 ドキュメント を 更新 し、情報 の 一貫性 を 保つ よう 配慮 してください。
