# ReceiveTimeout DelayQueue PoC 記録

## 区分基準
- **実行要約**: PoC の設定。
- **結果**: 計測値。
- **メモ**: 実装検討事項。


## 実行要約
- 日付: 2025-09-26 (JST)
- コマンド: `DELAYQUEUE_ITERATIONS=20000 DELAYQUEUE_TIMEOUT_MS=10 cargo run -p nexus-actor-core-rs --example receive_timeout_delayqueue`

## 結果サマリ
- 再アーム合計時間: 約 11.683 ms / 20,000 回
- 再アーム平均コスト: 約 584 ns/回
- 発火待ち時間: 約 11.406 ms (timeout 10 ms + poll オーバーヘッド)

## メモ
- DelayQueueTimer は `tokio_util::time::DelayQueue` のキーを保持し、`reset` でデッドラインのみ更新する構造。
- ActorContext へ統合する際は `ActorContextExtras::init_or_reset_receive_timeout_timer` から `DelayQueueTimer` を呼び出す想定。
- 既存 `ReceiveTimeoutTimer` 互換 API を維持するため、`reset/stop/wait` を同名で実装。
- 次段階: ActorContext へ DelayQueueTimer を注入し、Sleep ベース実装とのベンチ比較を docs/benchmarks/core_actor_context_lock.md に反映する。
