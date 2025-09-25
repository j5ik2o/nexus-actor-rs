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
- `ActorSystem::get_config()` は同期メソッドへ移行済み。今後は `metrics_foreach` 呼び出しの段階的置換と Supervisor/metrics 経路の完全同期化を継続し、Dispatcher ポリシーに沿った一貫したホットパス設計を完成させる。
