# Dispatcher Runtime ポリシー

`SingleWorkerDispatcher` をはじめとする Dispatcher 実装で Tokio Runtime を内部管理する場合の指針をまとめる。

## 背景
- 従来は `SingleWorkerDispatcher` が `tokio::runtime::Runtime` を `Arc` で保持し、Drop 時に暗黙に破棄していた。
- テスト終了時に `Runtime` が async 文脈上で drop されると `Cannot drop a runtime in a context where blocking is not allowed` エラーが発生しうる。

## ポリシー
1. **Runtime の明示的な解放**
   - Runtime を `Option<Arc<Runtime>>` で保持し、`Drop` 実装で `Arc::try_unwrap` に成功した場合 `shutdown_background()` を呼び出す。
   - `Arc::strong_count` が 1 でない場合、Dispatcher による所有権が無いとみなし、そのままドロップしてもよい（Runtime 解放は他の所有者に委譲）。

2. **再利用禁止**
   - Dispatcher を clone した場合でも `Arc<Runtime>` が共有される。`Drop` 時には `Option::take` を利用して二重 shutdown を避ける。

3. **ユニットテストにおける注意**
   - `Remote` テストなど長時間稼働するランタイムを保有するコンポーネントは、停止処理 (`shutdown`) の完了前に Dispatcher を drop しない。

4. **追加の Dispatcher 実装**
   - 新規 dispatcher が Runtime を内部管理する場合も同様のパターン (`Option<Arc<Runtime>>` + `shutdown_background`) を採用する。

## 実装例
- `core/src/actor/dispatch/dispatcher.rs` `SingleWorkerDispatcher`
  - `Drop` で `shutdown_background()` を呼び出すようになっている。
  - `schedule` は runtime が `Some` の時のみ `spawn` を実行する。

## 今後の展望
- Dispatcher を構築するためのヘルパーモジュールを提供し、Runtime 管理を一元化する。
- `CurrentThreadDispatcher` 等、Runtime を必要としない実装との API 平準化を検討する。

## ActorSystem 設定とメトリクス初期化の同期化（2025-09-25 PoC）
- `ActorSystem` の内部状態を `ArcSwap<Config>` / `ArcSwapOption<MetricsRuntime>` へ移行し、設定値とメトリクスランタイムをホットパスで取得できるようにした。`get_config()` も同期メソッド化され、`config_arc()` と合わせて await を挟まずに設定へアクセスできる。
- メトリクス拡張 (`Metrics`) は `ArcSwapOption<MetricsRuntime>` を受け取って初期化し、OpenTelemetry provider が有効な場合はランタイムをスロットへ格納する。これにより、拡張取得 (`extensions.get`) の `Mutex` を待たずに `MetricsRuntime` へアクセスできる。
- 新たに `ActorSystem::metrics_foreach`（同期クロージャ）と `Metrics::foreach`（同期版）を追加し、Supervisor や Context 経路で `await` を挟まずメトリクスをハンドリングできることを確認。`core/src/actor/actor_system/tests.rs::test_metrics_foreach_sync_access` で PoC を検証済み。
- `ActorContext` は `ArcSwapOption<MetricsRuntime>` を保持し、`metrics_sink()` がランタイム非存在時には即座に `None` を返し、存在時には `OnceCell` へ同期的にキャッシュする設計へ更新した。ランタイム差し替え後でも既存 Context から新しいスナップショットが参照可能。
- `ActorHandle` は型名を `Arc<str>` でキャッシュし、`type_name()` を同期 API 化。メトリクス初期化や Supervisor 経路で型名取得のために待機が発生しないようになった。
- `ContextHandle` は `ArcSwap<RwLock<Box<dyn Context>>>` を採用する PoC を導入済み。スナップショット (`ContextCell`) との併用でホットパスの参照取得は同期化されつつ、従来の `RwLock` により受信順序の保証を維持している。
- `ActorSystem::get_config()` は同期メソッドへ移行済み。Supervisor/metrics 経路は `record_supervisor_metrics` により同期パスへ統合済みであり、今後は ContextHandle まわりの ArcSwap PoC・イベント発火経路の同期化など残タスクに注力する。

### Protoactor-Go 呼び出し順序との差分ドラフト（2025-09-26）
- **メッセージ処理の順序**（protoactor-go `actor/context.go`）
  1. `Receive(envelope *MessageEnvelope)` で `ctx.messageOrEnvelope` にメッセージをセットし、`defaultReceive()` を同期的に呼び出す。
  2. `defaultReceive()` は `AutoRespond` や decorator の有無を判定し、`ctx.actor.Receive(ctx)` あるいは装飾済みコンテキストを直接渡す。
  3. 呼び出し完了後に必ず `ctx.messageOrEnvelope = nil` でクリアする。
  この設計では Actor は常に `'static` な `Context` 実装へ同期アクセスし、送信者・メッセージはフィールド経由で参照する。
- **nexus-actor-rs の対応**
  - `ContextHandle::receive` はメッセージを `ActorContext::set_message_or_envelope` に格納した後、`ContextBorrow<'_>` と `TypedContextSyncView` を介したライフタイム安全な参照を提供する。`sync_view()` により、ホットパスでは `try_get_message_handle_opt` / `try_get_sender_opt` / `try_get_message_header_handle` のスナップショットを取得し、フォールバックとして既存の async getter を呼び出す二段構えにしている。
  - decorator 相当の経路では `ContextBorrow` を短時間で解放するために `with_actor_borrow(|borrow| ...)` パターンを用い、長寿命の `ArcSwap` スナップショットを通じてライフタイム準拠を保つ。
- **スーパー ビジョンの順序**（protoactor-go `actor/supervision.go`）
  - Go 実装は `SupervisorStrategy.HandleFailure(actorSystem, supervisor, child, rs, reason, message)` を同期呼び出しし、`Supervisor` から `RestartChildren` や `StopChildren` を直ちに呼ぶ。
  - Rust 実装では `SupervisorHandle` が `ContextBorrow` 由来の `ExtendedPid` と `TypedContextSyncView` を引き渡し、`record_supervisor_metrics` でスナップショットを確保した後に非同期操作へ移行する。これにより、再起動・停止処理を await 可能な `StopperPart`／`SpawnerPart` API へ正しく委譲できる。
- **ライフタイム指向のガイドライン反映案**（ドラフト）
  1. ドキュメント化された呼び出し順序（上記）を基に、「メッセージ格納→借用→フォールバック await」の流れを Dispatcher / Supervisor 方針へ明示する。
  2. `ContextBorrow` を利用する箇所では、`Drop` 時に `message_or_envelope` をクリアする Go 流儀との互換性を保つため、`context_cell_stats()` を併せて計測し、await を伴う fallback がどの程度発生しているかを監視する。
  3. Supervisor では Go 実装との違い（即時同期呼び出し vs. ライフタイム尊重の2段階処理）を `handle_failure` 系 API のドキュメントへ追記し、Dispatcher には「同期スナップショットを先に試し、失敗時のみ async」を徹底するチェックリストを提示する。
