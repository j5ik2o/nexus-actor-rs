# actor モジュール分割タスク (2025-10-01 時点)

## 区分基準
- **ステータス別**: `完了済み` は既に実施済みのタスク、`継続タスク` は今後着手すべきもの、`検証・ドキュメント` は実装完了後に行う確認作業。

## 完了済み（2025-10-01）
- 既存 `modules/actor-core` を `modules/actor-std` へ移設し、Tokio/std 依存の本体・ベンチ・サンプルを `nexus-actor-std-rs` に集約。
- `modules/actor-core` を `#![no_std]` + `alloc` ベースの最小スケルトンに再構築し、コア API を再編する準備を完了。
- `actor-std` の Cargo 設定を整理し、従来の依存関係（Tokio, opentelemetry など）を引き継ぎつつ `nexus-actor-core-rs` に依存させる形へ更新。
- `actor-core` に `actor::core_types::message` モジュールを追加し、`Message`/`ReceiveTimeout`/`TerminateReason` などの no_std 対応な基盤型を定義。`actor-std` は同モジュールを再エクスポートし、標準実装向けヘッダー拡張などを追加する構成に変更。
- ランタイム抽象（AsyncMutex/RwLock/Notify/Timer）を actor-core に追加し、actor-std で Tokio 実装を提供。スタッシュ操作など `tokio::sync` 直参照部分をアダプタ越しに置換。
- EndpointWatcher／PidSet の非同期化を反映し、DashMap ガードと Future の衝突を解消済み。
- `cargo test --workspace` が新構成でも成功することを確認。
- EndpointWatcher の監視操作ヘルパーを共通化し、Criterion ベンチ `endpoint_watch_registry` を追加して PidSet 操作のレイテンシ計測を開始。

## 継続タスク（優先度：高→低）
- 【高：監視拡張】EndpointManager / EndpointSupervisor など監視イベントを仲介する層でヘルパー API を適用し、watch/unwatch/terminate の分岐を一本化する。必要に応じて `WatchedRegistry` 抽象を導入する。
- 【中：抽象再設計】ロック／タイマー／チャネル等の Tokio 依存箇所を抽象化し、actor-core では trait のみに集約、actor-std が Tokio 実装を提供する構造へ段階的に移行する（対象：mailbox, process, supervisor など）。
- 【中：コア移植】`alloc` だけで動くコンポーネント（PID, middleware, Serialized message handles など）を actor-core に移し、必要に応じて `alloc::` 系型や `hashbrown` への置き換えを実施する。

## 検証・ドキュメント（優先度順）
1. `cargo check -p nexus-actor-core-rs --no-default-features --features alloc` を追加し、actor-core 単体の no_std ビルドが通るタイミングを随時確認。
2. `cargo test --workspace` と `cargo bench -p nexus-actor-std-rs` を継続実行し、分離作業による回帰を監視。
3. `docs/` 配下（`core_improvement_plan.md` など）を actor-core / actor-std の役割に合わせて更新し、作業完了段階で MECE に整理。
4. 変更内容を整理したリリースノート草案を作成し、依存プロジェクトへの影響範囲（新たな `nexus-actor-core-rs` の位置付け）を明記する。
