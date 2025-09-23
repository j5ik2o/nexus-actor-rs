# Repository Guidelines

CLAUDE.mdも参照すること。

## プロジェクト構成とモジュール
本 リポジトリ は Cargo ワークスペース。主要 ディレクトリ は 以下 の 通り。
- `core/` アクター ランタイム と メッセージ 処理。テスト は モジュール 直下 の `tests.rs`。
- `cluster/` メンバーシップ と Gossip。生成 物 は `cluster/generated/`。
- `remote/` gRPC ベース の リモート メッセージング。
- `message-derive/` メッセージ 派生 マクロ 定義。
- `utils/` 共通 ヘルパー と キュー 構造。
共有 設定 は ルート `Cargo.toml` と `rust-toolchain.toml`。ビルド 自動化 は `Makefile.toml`、カバレッジ は `coverage.sh` を 使用。

## ビルド・テスト・開発コマンド
- `cargo build --workspace` : 全 クレート を ビルド。
- `cargo test --workspace` : 全 テスト 実行。部分 実行 は `cargo test -p core actor::dispatch::tests::`。
- `cargo clippy --workspace --all-targets` : Lint 警告 0 を 維持。
- `cargo +nightly fmt` : `rustfmt.toml` (max_width 120、tab_spaces 2) に 従い 整形。
- `cargo make coverage` / `./coverage.sh` : grcov HTML を `target/coverage/html/index.html` に 出力。

## コーディングスタイルと命名規約
ファイル と 関数 は snake_case、型 と トレイト は PascalCase、定数 は SCREAMING_SNAKE_CASE。非同期 処理 は `tokio` と `async-trait` を 前提 と し、`?` で 早期 戻り を 心掛けます。`cargo +nightly fmt` と `cargo clippy` を PR 前 の 必須 チェック と し、`tracing` ログ は デバッグ 範囲 に 留めて ください。

## テストガイドライン
テスト フレームワーク は Rust 標準 + `#[tokio::test]`。関数 名 は `test_<対象>_<期待>` の snake_case を 推奨。共有 状態 は `Arc`、`AsyncBarrier`、`Notify` など を 使用 し データ 競合 を 回避。重要 シナリオ は 正常 系 と エラー 系 を 両方 カバー し、必要 に 応じて `cargo make coverage` の 成果 を PR に 添付。

## コミットと Pull Request ガイドライン
コミット メッセージ は `<type>: <要約>` (例 `refactor: clean up dispatcher tests`) を 基準。PR 説明 には 背景、変更 点、実行 済み コマンド (`cargo test`、`cargo clippy`)、関連 Issue を 箇条書き。挙動 が 変わる 場合 は ログ や スクリーンショット を 添付 し 再現 手順 を 明示。

## セキュリティと設定のヒント
リモート 接続 を 試験 する 際 は `RUST_LOG=debug` を 設定 し、シークレット は `.env` など 非公開 設定 に 保管。`coverage.sh` が 依存 する `grcov` と `llvm-tools-preview` は `rustup component add` / `cargo install` で 同期。CI と ローカル の ツール バージョン が 合致 している こと を 定期 的 に 確認 してください。
