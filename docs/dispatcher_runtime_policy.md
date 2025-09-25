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
