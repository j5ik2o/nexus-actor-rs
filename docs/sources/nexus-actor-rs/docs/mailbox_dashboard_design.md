# Mailbox ダッシュボード設計ノート (2025-09-29 時点)

## 区分基準
- **メトリクス仕様**: Exporter が公開する指標とラベル体系。
- **可視化/アラート**: Grafana などで利用するダッシュボードと通知ルール。
- **フォローアップ**: 未実装または検討中の項目。

## メトリクス仕様
| メトリクス | 種別 | 主ラベル | 備考 |
|------------|------|-----------|------|
| `nexus_actor_mailbox_queue_dwell_duration_seconds` | Histogram | `queue_kind`, `actor_type`, `address` | `modules/actor-core/src/metrics/actor_metrics.rs:70-88` で定義。 |
| `nexus_actor_mailbox_queue_dwell_percentile_seconds` | Gauge | `queue_kind`, `percentile`, `actor_type`, `address` | Snapshot を `DefaultMailbox::queue_latency_metrics()` から記録。 |
| `nexus_actor_mailbox_queue_length` | Gauge | `queue_kind`, `actor_type`, `address` | `record_mailbox_queue_length` でキュー長を即時記録。 |
| `nexus_actor_mailbox_suspension_state` | Gauge | `actor_type`, `address` | Suspend(1)/Active(0)。 |
| `nexus_actor_mailbox_suspension_duration_seconds` | Histogram | `actor_type`, `address` | Suspend 継続時間。 |
| `nexus_actor_mailbox_suspension_resume_count` | Counter | `actor_type`, `address` | Resume 発生回数。 |

スナップショット間引き (`queue_latency_snapshot_interval`) は exporter 側でメタラベル化していないため、ダッシュボードの説明欄に記載する。

## 可視化/アラート
- **Queue Percentiles**: `nexus_actor_mailbox_queue_dwell_percentile_seconds{queue_kind="user",percentile="p95"}` を 5 分窓で可視化。Warning=5ms, Critical=20ms 継続 3 分を暫定閾値。
- **Queue Length Heatmap**: `nexus_actor_mailbox_queue_length` をアクター種別ごとにヒートマップ表示。Warning=1024 継続 30 秒、Critical=4096 継続 3 分。
- **Suspension Overview**: `nexus_actor_mailbox_suspension_state` の 60 秒超継続を Warning。`nexus_actor_mailbox_suspension_resume_count` の 10 分窓 rate > 50 を頻発アラートとする。
- **Dashboard JSON**: Grafana 用ダッシュボードは未公開だが、`docs/bench_dashboard_plan.md` からリンク予定。

通知フロー: Warning は Slack 運用チャンネル、Critical は PagerDuty。自動解消 10 分ルールを適用し、急減少時は通知解除。

## フォローアップ
- Snapshot interval をラベル化する exporter 拡張案を検討（`MailboxQueueLatencyMetrics` にラベル付与余地あり）。
- Remote 専用キュー (`EndpointWriterMailbox`) のメトリクスを `actor_type="remote"` で区別するか、専用ラベル `pipeline=remote` を追加するか検討。
- Grafana ダッシュボード JSON をリポジトリに追加し、レビュー用 PR テンプレートへ添付する運用を整備。

