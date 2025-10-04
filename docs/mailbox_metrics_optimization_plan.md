# Mailbox メトリクス最適化メモ (2025-09-29 時点)

## 区分基準
- **完了済み施策**: main に取り込まれた最適化とその確認ポイント。
- **残課題**: 今後の調整・PR が必要なタスク。
- **リスクと監視**: 運用上の留意点。

## 完了済み施策
- `DefaultMailbox` に `should_emit_latency_update` を導入し、`queue_latency_snapshot_interval` ごとのサンプリングに集約（`modules/actor-core/src/actor/dispatch/mailbox/default_mailbox.rs:470-520`）。ベンチ `test_default_mailbox_emits_latency_metrics_with_interest` で検証済み。
- Queue length Gauge を enqueue/dequeue 双方で更新し、`record_mailbox_queue_length` がホットパスをロックレスで通過（`modules/actor-core/src/actor/metrics/metrics_impl.rs:200-370`）。
- `MailboxMetricsCollector` を追加し、`ConfigOption::with_mailbox_metrics_poll_interval` で収集間隔を制御（`modules/actor-core/src/actor/metrics/metrics_impl.rs:60-180`）。
- EndpointWriterMailbox にサンプリング間隔を適用し、`ConfigOption::with_endpoint_writer_queue_snapshot_interval` で制御可能。`remote/src/tests.rs::endpoint_writer_queue_snapshot_interval_samples_updates` で回帰テスト済み。
- Nightly / Weekly ベンチでメトリクス有効時のオーバーヘッドを継続監視（`.github/workflows/mailbox-sync-nightly.yml`）。

## 残課題
- `remote/src/config.rs` の `endpoint_writer_queue_snapshot_interval` 既定値は依然 1。運用推奨値（本番 32, staging 8, dev 1）へ更新し、Runbook へ反映する。
- `queue_latency_snapshot_interval` の環境別デフォルトを `Config` 経由で外出しし、SRE チームと合意した 64/8/1 をコード化する（現状は DefaultMailbox 側の定数設定のみ）。
- メトリクス比較用の Golden テスト（`core/tests`）を追加し、間隔変更時の `nexus_actor_mailbox_queue_dwell_percentile_seconds` 出力を検証する。

## リスクと監視
- サンプリング間隔を広げると短時間のスパイクを検知できない可能性があるため、Grafana の説明欄に `queue_latency_snapshot_interval` を明記し、Critical 閾値（p95 > 20ms 3 分継続）でアラートを維持。
- `MailboxMetricsCollector` は Tokio ランタイム上で動作するため、`SystemMetricsSink` 未設定環境では Collector 起動をスキップ（`metrics_impl.rs:50-90`）。構成変更時は `METRICS_ENABLED` の設定漏れに注意。

