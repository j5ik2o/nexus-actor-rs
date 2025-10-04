# ディスパッチャ経路メトリクス設計メモ

## 区分基準
- **目的/計測ポイント**: 何を計測するか。
- **実装方針**: どう組み込むか。
- **未決事項**: 残課題。


## 目的
- ActorContext が dispatcher に処理を委譲する際の待ち時間とキュー滞留を可視化し、ロック削減施策の効果を数値化する。
- メールボックス投入から invoker による取り出しまでの経路を計測し、ホットパスのボトルネック（キュー詰まり・スケジューラ遅延）を特定する。

## 計測ポイント
1. **Mailbox への投入 (`DefaultMailbox::offer_*`)**
   - user/system 両方のキュー長を `MetricsSink::record_mailbox_length_with_labels` で記録。
   - 投入時刻を `MessageHandle` 拡張メタデータで保持し、取り出し時にディスパッチ遅延を算出。
2. **Dispatcher `schedule` 呼び出し (`Dispatcher::schedule`)**
   - `Runnable` 実行開始までの遅延を `Histogram(actor_dispatch_start_latency_ms)` として記録。
   - `InstrumentedRwLock` と合わせて lock wait trace に相関させる。
3. **Invoker 実行前 (`MessageInvoker::invoke_user_message`)**
   - メールボックス取り出し時点と Actor ハンドラ実行完了までの時間を `message_processing_duration` として測定。

## 収集するメトリクス案
| 種別 | 名前 | ラベル | 説明 |
| --- | --- | --- | --- |
| Gauge | `actor_mailbox_backlog` | `{actor_type, queue_type=user/system}` | 投入直後のキュー長。
| Histogram | `actor_dispatch_latency_ms` | `{actor_type}` | `schedule` から executor 実行までの遅延。
| Histogram | `actor_mailbox_residence_ms` | `{actor_type, message_type}` | Mailbox 投入から invoker 処理開始までの時間。
| Counter | `actor_dispatch_drops_total` | `{actor_type}` | スケジューラ確保に失敗した回数（`QueueError::Full` 等）。

## 実装方針
- `MessageHandle` に `DispatchTiming` 構造体をオプションでぶら下げ、投入時刻・ dispatcher submit 時刻を格納する。既存構造を壊さないように feature flag (`lock-metrics` 同等) で切り替え可能にする。
- `DefaultMailbox` の `offer_*` / `poll_*` に計測コードを追加し、`MetricsSink` を取得できるよう `ActorContextExtras` から `MailBox` へアクセスする補助関数を導入。
- 測定値は `ActorContext` 内にある `MetricsSink` を利用し、Provider 未設定時は no-op。

## ベンチ計測
- `modules/actor-core/benches/actor_context_lock.rs` を拡張し、以下を計測する Criterion ベンチを追加：
  - 高負荷時の `actor_dispatch_latency_ms` の分布
  - キュー長（メールボックスバッファ）が一定値を超えるまでの時間
- ベンチ結果は `docs/benchmarks/core_actor_context_lock.md` に追記し、リグレッション検出に活用。

## 未決事項
- DelayQueue 置換との整合: `ReceiveTimeoutTimer` で DelayQueue を採用した場合、タイマーイベントにも同じメトリクスを適用するか要検討。
- `QueueWriter` へ統一したため、計測ロジックは同期 API 前提で挿入する。旧 async 実装との差分を整理し、回帰テスト方針を更新する。
- 運用メトリクスとベンチ用途を切り分けるため、`metrics::actor_dispatch_latency_ms` には export 先ごとのバケット設計を決める必要がある。

## 次のアクション
1. `MessageHandle` 拡張と Mailbox 計測コードの PoC 実装。
2. ベンチマーク (`actor_context_lock.rs`) に滞留時間算出ロジックを追加し、基準値を取得。
3. 取得したメトリクスを `docs/benchmarks/tracing_actor_context.md` に統合して可視化フローを整理。
