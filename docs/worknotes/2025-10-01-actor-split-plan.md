# actor モジュール分割タスク (2025-10-01 時点)

## 区分基準
- **ステータス別**: `完了済み` は既に実施済みのタスク、`継続タスク` は今後着手すべきもの、`検証・ドキュメント` は実装完了後に行う確認作業。

## 完了済み（2025-10-02 更新）
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
- CoreContextSnapshot に parent／actor／シリアライズ情報を保持する抽象を追加し、std 側からのスナップショット capture でも同情報を供給できるよう調整（2025-10-03）。
- ActorContext のメッセージ API を `MessageEnvelope` ラッパ経由に整理し、CoreMessageEnvelope と整合する変換経路を確立。
- Supervisor／SupervisorStrategy トレイトを CorePid ベースへ移行し、`CorePidRef` 抽象と `ExtendedPid` 実装を整備して、監視系メトリクス・イベントの橋渡しを簡潔化。
- ExtendedPid ↔ CorePid 変換の共通ヘルパー（`From` 実装・スライス変換ユーティリティ）を追加し、監視経路・コンテキストからの PID 変換処理を一元化。
- SystemMessage（Restart/Start/Stop/Watch/Unwatch/Terminate）を actor-core の `core_types::system_message` に集約し、std 層では ProtoBuf 変換ヘルパーと runtime 実装のみを保持する構成へ移行。
- ProcessRegistry を `CoreProcessRegistry` として trait 化し、core 層がインターフェース、actor-std が tokio/DashMap 実装を提供する構造に更新。
- Mailbox の最小操作を `CoreMailbox` トレイトとして切り出し、actor-std の `MailboxHandle` が core 抽象を実装するように整備。
- Mailbox queue ハンドルを `CoreMailboxQueueHandle` でラップし、`SyncMailboxQueueHandles` から core 抽象を取得できるアダプタを追加。
- メトリクス側で `core_queue_handles()` を利用し、キュー長を CoreMailboxQueue 経由で観測するように更新。
- RingQueue を `RingBuffer` ベースへ移行し、std 層は `Mutex` 包装のみを担当する構造に整理。
- DefaultMailbox 内のキュー長取得も CoreMailboxQueue ベースに切り替え、dispatch ループでの統計取得を core 抽象へ統一。
- CoreMailboxQueueAsyncExt を追加し、キュー操作を Future 化できるインターフェースを core レイヤで提供。
- CoreActorContextSnapshot に builder と `receive_timeout` フィールドを追加し、Std 側が core 抽象へ同期スナップショットを供給できるよう整備（2025-10-03）。
- ContextExtensions を core 抽象 (`CoreContextExtensions`) として再実装し、std 層は Tokio RwLock を注入する薄いアダプタのみとした（2025-10-03）。
- 2025-10-01 に `cargo check -p nexus-actor-core-rs --no-default-features --features alloc`、`cargo test --workspace`、`cargo bench -p nexus-actor-std-rs` を実行し、actor-core/no_std 経路と actor-std ベンチの回帰が無いことを確認。併せて docs 配下の役割整理とリリースノート草案を更新。
- EndpointSupervisor で EndpointManager の既存 `WatchRegistry` を再利用し、再起動時も監視スナップショットを維持するよう更新。`RemoteProcess` / `EndpointManager` 間で `WatchRegistry` を参照して重複 watch/unwatch を抑制し、イベント／テレメトリとワーカーメッセージ送信を整合。
- CoreMailboxQueue を trait object 化し、MPSC／Ring／Priority ベースキューを `CoreMailboxQueue` に直結するアダプタを導入して、SyncMailboxQueueHandles から共通取得できるよう統一。
- Supervisor 向けに `ErrorReasonCore`・`CoreSupervisor*` トレイト族を actor-core へ追加し、tokio 依存を std 層アダプタへ閉じ込める準備（設計メモ `docs/design/2025-10-01-supervisor-trait-refactor.md` 作成）。
- dispatch ベンチマークを CoreSchedulerDispatcher ベースへ更新し、TokioRuntimeContextDispatcher の残存依存を除去。Props → CoreMailbox 経路の CoreMailboxFactory 利用をユニットテストで検証し、CoreMailbox 契約を破綻なく再確認。
- ActorContext／Guardian の障害処理を CoreSupervisorStrategy / CoreSupervisor 経由に統合し、RestartStatistics のクロック情報を保持したままコアトラッカーと往復させるアダプタを整備。ガーディアン経路・ルート監督でも CoreRuntime 依存に揃え、再起動閾値の判定が tokio 非依存で成立することを確認。
- DefaultMailbox のタスク協調を `poll_fn` ベースのランタイム非依存 `yield` に置換し、Throttler／Dispatcher の TokioMutex 依存も runtime 抽象経由に統一。CoreMailboxQueue のテストを追加して core 抽象からのメールボックス操作を検証。
- SingleWorkerDispatcher の tokio ランタイム生成を `runtime::build_single_worker_runtime()` に集約し、Tokio runtime ライフサイクル管理を専用 helper へ委譲。Dispatcher や Mailbox 経由の Tokio sync primitives は `StdAsyncMutex` / `StdAsyncRwLock` 再エクスポートを通じて参照する構成に整理。
- CoreRuntimeConfig／CoreRuntime に AsyncYield を取り込み、TokioRuntime から yielder 抽象を渡す経路を整備。`runtime_yield_now()` も core 抽象経由の yield を優先し、Mailbox／Dispatcher の協調実行を Tokio 依存から切り離した。
- Props 経由の MailboxFactory で CoreRuntime の AsyncYield を Mailbox に注入し、DefaultMailbox／MailboxHandle 双方がランタイム非依存な協調実行を選択できるよう整備。AsyncYield を利用した協調実行テストを追加し、CoreMailboxFactory の契約が維持されることを検証。
- MailboxMetricsCollector を CoreScheduler 駆動へ移行し、Tokio `spawn`/`interval` 依存を排除。CoreRuntime に同期したポーリングとハンドルキャンセルのテストを追加し、Supervisor／ガーディアン経路のランタイム抽象化を完了。
- RestartStatistics を CoreRuntime の FailureClock から初期化するルートを追加し、`with_runtime` テストで再起動ウィンドウがコア抽象に基づいて進むことを検証。Tokio 依存のタイムソースを排除し CoreRuntime 提供の時計に一元化。
- OneForOneStrategy の Supervisor テストを拡充し、CoreRuntime 由来の FailureClock で再起動ウィンドウ閾値が判定されることをシナリオ単位で検証。
- StdSupervisorContext へ CoreRuntime の FailureClock を保持させ、RestartStatistics が CoreRestartTracker から直接 InstantFailureClock を再利用できるように拡張。SupervisorAdapter 周りのテストを追加し、Mailbox／Supervisor 抽象がランタイム非依存で動作することを確認。
- AllForOneStrategy／ExponentialBackoffStrategy を CoreRuntime FailureClock 前提に再テストし、時間窓を跨いだ再起動判定とバックオフ更新がコアクロックで安定することを確認。
- SupervisorStrategyHandle を CoreSupervisorStrategyHandle ベースへ置き換え、OneForOne／AllForOne／ExponentialBackoff／Restarting 各戦略を CoreSupervisorStrategy 実装に移行。関連テストを CoreRuntime 経由で呼び出すよう更新し、remote 側は標準戦略ハンドルを利用する構成に整理。
- Mailbox／Supervisor 系の Tokio 依存を core 抽象＋std アダプタへ集約。Mailbox は CoreMailbox トレイトと CoreMailboxFactory で統合し、Supervisor は CoreSupervisorStrategy へ一本化。std 層は `StdSupervisorContext`／`StdSupervisorAdapter`／Tokio 同期プリミティブのみを提供し、remote/guardian も共通ハンドル利用へ移行。
- Props が `CoreProps` を内部に保持するよう再構成し、actor-std の拡張設定（メールボックス生成・ミドルウェア・メトリクス）が core 抽象を通じて供給されるよう統一。`core_props()` は保持済み CoreProps を返し、no_std 層が必要とする最小 API を actor-core で完結させた。
- MessageHandles と PidSet を CoreMessageHandles/CorePidSet として actor-core へ移植。std 層は Tokio ベースの `AsyncMutex`／`AsyncRwLock` を注入するラッパーのみとなり、alloc 環境でも共通ロジックが利用可能に。
- CoreActorError・CoreReceiverInvocation と MessageEnvelope の Core⇔Std 変換 API を整備し、ReceiverSnapshot から core 呼び出しデータを直接得られるようにして Props／ActorContext の no_std 化基盤を拡充（2025-10-02）。
- Receiver／Sender／Spawn middleware を CoreMiddleware 抽象（CoreReceiverMiddlewareChain/CoreSenderMiddlewareChain/CoreSpawnMiddleware）へ移行し、actor-std 側は context スナップショットや ExtendedPid 変換を担う薄いアダプタのみに整理（2025-10-02）。

## 継続タスク（優先度：高→低）
- （高優先度タスクなし：2025-10-03 時点。ActorContext／Props 最小核の移植は完了。）
- 【中：コア移植】`alloc` だけで動くコンポーネント（Serialized message handles など）を actor-core に移し、必要に応じて `alloc::` 系型や `hashbrown` への置き換えを実施する。

## 検証・ドキュメント（優先度順）
- 現状タスクなし（2025-10-01 時点）。CI 定期実行とリリースノート仕上げフェーズへの移行待ち。
