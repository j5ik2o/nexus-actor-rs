# DefaultMailbox::run 再構成メモ (2025-09-28)

## 現状整理
- system/user メッセージ処理は `try_handle_system_message` / `try_handle_user_message` でヘルパ化済み。ロック解放後に `await` するパターンを徹底。
- ループ制御は `dispatcher.throughput().await` から取得した値 `t` とカウンタ `i` で行い、`i >= t` で `tokio::task::yield_now()` を呼び出している。
- `suspended` フラグは `AtomicBool`。system メッセージで Suspend/Resume を受け取るたびに更新する。

## 課題・改善アイデア
1. **`yield_now` の発火条件**
   - 現状: `should_yield(iteration, throughput, system_pending, user_pending)` で backlog がゼロでない場合のみ `yield_now` を呼ぶ構造に変更済み。`last_backlog` で増加傾向かどうかも確認するようになった。
   - TODO: `should_yield` を `should_yield(iteration, throughput, system_pending, user_pending, dispatcher_hint)` に拡張し、Dispatcher からの過負荷シグナルや backlog 変化率をより柔軟に判定へ組み込む。
2. **フェアネスとバックプレッシャー**
   - 課題: system キュー連続処理で user キューが枯渇するリスクは依然ある。
  - TODO1: system:user = 1:N の比率を設定値として導入し、`Mailbox` 側で制御できるようにする。
  - TODO2: `user_messages_count` / `system_messages_count` の比率から優先度を切り替えるダイナミック・フェアネスを検討。
  - TODO3: Suspend 状態が連続する場合の挙動（短時間 sleep、強制 resume 等）を仕様化する。
3. **Dispatcher との協調**
   - TODO: `DispatcherHandle::schedule` 以外で制御を戻す手段（アイドル通知・yield 指示など）を検討し、Mailbox と Dispatcher の連携を強化する。
4. **メトリクス**
   - `QueueLatencyTracker` は `std::time::Instant` ベースで同期ロックのみを使用する構成に統一済み。
   - Suspend/Resume 発生回数と累積停止時間は `MailboxSuspensionMetrics` に記録され、`DefaultMailbox::suspension_metrics()` から取得可能。
   - `LatencyHistogram` を導入し、`DefaultMailbox::queue_latency_metrics()` から user/system 双方のヒストグラムスナップショットとパーセンタイルを算出可能にした。
   - `ActorMetrics` の Gauge `nexus_actor_mailbox_queue_dwell_percentile_seconds` に `queue_kind` + `percentile` ラベル付きで `p50`/`p95`/`p99` を記録するフックを追加。
   - TODO: queue 滞留時間のヒストグラムを OTEL にエクスポートし、yield 発生回数のメトリクス化を検討。
5. **テスト/ベンチ**
   - system/user 偏り、Suspend/Resume 多発の統合テストを用意し、フェアネス制御が機能するか検証する。
   - `mailbox_throughput` を拡張し、system メッセージ混在や大 backlog シナリオを測定する。

## 次のタスク候補
- `should_yield` 判定を拡張し、Dispatcher からのヒントや backlog 変化率を考慮する。
- フェアネス制御ポリシー（system:user 比率、バックプレッシャー時の挙動）をドキュメント化し、実装候補を比較する。
- メトリクス API の TokIo 依存解消案を検討し、`record_queue_*` の改善タスクを切り出す。
- ベンチとテストの拡張（system 混在、Suspend/Resume シナリオ）を設計し、回帰防止に備える。
