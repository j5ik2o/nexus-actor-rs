# actor モジュール分割タスク (2025-10-01 時点)

## 区分基準
- **準備**: 着手前に依存と影響を把握するための調査タスク。
- **実装タスク**: actor-core / actor-std への再編そのもの。優先度の高い順に記載。
- **検証・ドキュメント**: 実装完了後に実施する確認・周知。優先度の高い順に記載。

## 準備（依存と影響調査）
1. `rg "tokio" modules/actor -n` / `rg "std::" modules/actor -n` で std / tokio 依存箇所を棚卸しし、core に残せない部分を洗い出す。
2. `cargo tree -p nexus-actor-core-rs` を実行し、現在の依存が `tokio`, `parking_lot`, `tonic` 等にどう繋がっているかを整理する。
3. actor モジュール内のテスト／ベンチ（`modules/actor/{benches,tests}`）で tokio ランタイム前提のものをリストアップし、後に actor-std へ移す対象を明確化する。

## 実装タスク（優先度順）
1. **クレート構成の再編**
   - `modules/actor` を `modules/actor-core` へリネームし、Cargo.toml の `package.name` を `nexus-actor-core-rs` に維持したまま `path` を更新する。
   - 新規に `modules/actor-std` を追加し、既存の `tokio` / `std` 依存ロジックをこちらへ移す土台となる `Cargo.toml` と `lib.rs` を作成する。
2. **機能分割**
   - `tokio`・`parking_lot`・`std::net` 等に依存するコードを特定し、 actor-std 側へ移設。
   - `no_std` で動作させたいコア部分（アクタートレイト、メッセージ、PID、スーパーバイザ、同期キュー経由のディスパッチャ等）を actor-core に残し、必要であれば `cfg(feature = "std")` を外す。
   - actor-core と actor-std 間の共有インターフェース（例: Mailbox/Dispatcher trait）の抽象化ポイントを整理し、`nexus-actor-std-rs` が `nexus-actor-core-rs` を拡張する構造にする。
3. **ワークスペース更新**
   - ルート `Cargo.toml` の `[workspace]` へ `modules/actor-core`, `modules/actor-std` を追加し、既存の依存先が新しいパスを指すよう調整する。
   - `modules/remote`, `modules/cluster` 等で `nexus-actor-core-rs` に依存していた箇所を再確認し、必要であれば `nexus-actor-std-rs` を追加。
4. **テスト／ベンチ移行**
   - `tokio::test` ベースのテストと Criterion ベンチを actor-std に移し、actor-core 側は `no_std` でコンパイル可能な最小限のテストに絞る。

## 検証・ドキュメント（優先度順）
1. `cargo check -p nexus-actor-core-rs --no-default-features --features alloc` を実行し、actor-core 単体で no_std ビルドが通るか確認する。
2. `cargo test --workspace` と `cargo bench -p nexus-actor-std-rs` を実行し、統合後も既存の挙動と性能測定が成立するか検証する。
3. `docs/` 配下の関連ドキュメント（例: `core_improvement_plan.md`, `mailbox_*` 系）を actor-core / actor-std の役割に合わせて更新し、変更点を MECE に整理する。
4. 変更内容をまとめたリリースノート草案（CHANGELOG もしくは docs/worknotes）を作成し、依存プロジェクトへの影響を明記する。
