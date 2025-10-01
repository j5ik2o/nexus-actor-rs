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
- EndpointManager が `WatchRegistry` を共有し、`remote_watch`/`remote_unwatch`/`remote_terminate` でもヘルパーを介した更新になるよう統一。
- EndpointWatchEvent をメトリクス基盤へ連動し、watch/unwatch/terminate イベントと監視数をエンドポイント × ウォッチャ単位で観測可能にした。
- AutoReceiveMessage と Terminated 情報を core に移し、std 層では ProtoBuf との変換のみを担当する構成へ整理。
- `Response` / `ResponseHandle` を actor-core へ移行し、std 層は再エクスポートのみとした。
- `MessageHandle` を actor-core に移し、メッセージ抽象の大半が no_std + alloc で完結する構成に整理。
- `ReadonlyMessageHeaders` と `ReadonlyMessageHeadersHandle` を actor-core に移設し、std 側では DashMap ベース実装で trait を満たす形へ整理。
- `CorePid` と `CoreMessageEnvelope` を導入し、PID／メッセージ封筒の純データ表現を actor-core に集約。std 側では ProtoBuf との相互変換と Tokio 依存処理のみを担うよう再編。
- `ProcessHandle` を `CoreProcessHandle` トレイトでラップし、CorePid ベースでの送受信 API を用意して Tokio 依存を std 実装に閉じ込めた。
- ProcessRegistry の AddressResolver を CorePid 入力へ切り替え、remote ハンドラ登録もコア抽象経由で行うよう更新。
- ActorContext のメッセージ API を `MessageEnvelope` ラッパ経由に整理し、CoreMessageEnvelope と整合する変換経路を確立。
- Supervisor／SupervisorStrategy トレイトを CorePid ベースへ移行し、`CorePidRef` 抽象と `ExtendedPid` 実装を整備して、監視系メトリクス・イベントの橋渡しを簡潔化。
- ExtendedPid ↔ CorePid 変換の共通ヘルパー（`From` 実装・スライス変換ユーティリティ）を追加し、監視経路・コンテキストからの PID 変換処理を一元化。
- SystemMessage（Restart/Start/Stop/Watch/Unwatch/Terminate）を actor-core の `core_types::system_message` に集約し、std 層では ProtoBuf 変換ヘルパーと runtime 実装のみを保持する構成へ移行。
- ProcessRegistry を `CoreProcessRegistry` として trait 化し、core 層がインターフェース、actor-std が tokio/DashMap 実装を提供する構造に更新。
- Mailbox の最小操作を `CoreMailbox` トレイトとして切り出し、actor-std の `MailboxHandle` が core 抽象を実装するように整備。
- Mailbox queue ハンドルを `CoreMailboxQueueHandle` でラップし、`SyncMailboxQueueHandles` から core 抽象を取得できるアダプタを追加。
- メトリクス側で `core_queue_handles()` を利用し、キュー長を CoreMailboxQueue 経由で観測するように更新。

## 継続タスク（優先度：高→低）
- 【高：監視拡張】EndpointSupervisor や RemoteProcess を含む監視経路で `WatchRegistry` を活用し、テレメトリや監視イベント発火との整合性を取る（例：EndpointSupervisor 経由の登録、RemoteProcess の最適化）。
- 【高：抽象再設計】Mailbox／Supervisor 系の Tokio 依存を trait 化し、actor-core がインターフェース、actor-std が Tokio 実装という責務分割に仕上げる（チャネル実装の差し替え方針も併せて整理）。
- 【高：ActorContext／Props 最小核】ActorContext・Props のうち同期／no_std で必要な API を actor-core に定義し、std 層はメールボックス・メトリクス拡張へ専念できる構造へ段階的に移行する。
- 【中：再起動統計】`RestartStatistics` 抽象化ドラフト（docs/worknotes/2025-10-01-restart-statistics-plan.md）に沿って、FailureClock トレイト導入とコア移植を進める。
- 【中：コア移植】`alloc` だけで動くコンポーネント（PID, middleware, Serialized message handles など）を actor-core に移し、必要に応じて `alloc::` 系型や `hashbrown` への置き換えを実施する。

## 検証・ドキュメント（優先度順）
1. `cargo check -p nexus-actor-core-rs --no-default-features --features alloc` を追加し、actor-core 単体の no_std ビルドが通るタイミングを随時確認。
2. `cargo test --workspace` と `cargo bench -p nexus-actor-std-rs` を継続実行し、分離作業による回帰を監視。
3. `docs/` 配下（`core_improvement_plan.md` など）を actor-core / actor-std の役割に合わせて更新し、作業完了段階で MECE に整理。
4. 変更内容を整理したリリースノート草案を作成し、依存プロジェクトへの影響範囲（新たな `nexus-actor-core-rs` の位置付け）を明記する。
