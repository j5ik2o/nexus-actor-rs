# core 最適化メモ (2025-09-29 時点)

## 区分基準
- **完了済み最適化**: 既に main に取り込まれた改善。
- **継続モニタリング**: 今後も計測・検証が必要な項目。

## 完了済み最適化
- `modules/actor-core/src/actor/dispatch/mailbox/default_mailbox.rs` は同期キュー実装と `QueueLatencyTracker`、`MailboxSuspensionMetrics` を統合済み。`should_yield` 判定は Dispatcher ヒントと backlog 感度で制御。
- `ContextHandle` 系 API は `ContextSnapshot` / `ContextBorrow` の同期参照を提供し、`ActorContextExtras` のホットパスから `Arc<Mutex<_>>` を排除済み。
- `bench.yml` と `bench-weekly.yml` が `reentrancy` / `context_borrow` ベンチを定期実行し、性能回帰の検出を自動化。

## 継続モニタリング
- Virtual Actor 経路（`cluster/src/virtual_actor/runtime.rs`）が DefaultMailbox のキュー特性に与える影響を追跡。必要に応じて mailbox throughput 設定を調整。
- DelayQueue 置換（`docs/benchmarks/receive_timeout_delayqueue.md` 参照）のベンチ再計測を実施し、receive timeout 系ロック待ちの改善度を測る。

