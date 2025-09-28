# Mailbox Dashboard & Alert Design (Draft)

## 1. 目的
- DefaultMailbox / EndpointWriterMailbox のキュー滞留・サスペンド状況をリアルタイムに可視化し、異常兆候を早期検知する。
- スナップショット間引き (`queue_latency_snapshot_interval`) を考慮したメトリクスの読み方を整理し、ダッシュボード利用者が解釈に迷わないようにする。
- suspend/resume 指標に対するアラートしきい値を暫定設定し、SLO/SLA に基づいた通知フローを確立する。

## 2. 対象メトリクスとラベル方針
| メトリクス | 意味 | 主なラベル |
|-------------|------|------------|
| `nexus_actor_mailbox_queue_dwell_duration_seconds` (histogram) | メールボックスでの滞留時間 | `queue_kind` (user/system), `actor_type`, `address` |
| `nexus_actor_mailbox_queue_dwell_percentile_seconds` (gauge) | 滞留時間の p50/p95/p99 | `queue_kind`, `percentile`, `actor_type`, `address` |
| `nexus_actor_mailbox_suspension_state` (gauge) | サスペンド状態 (1=suspended) | `actor_type`, `address` |
| `nexus_actor_mailbox_suspension_duration_seconds` (histogram) | サスペンド継続時間 | `actor_type`, `address` |
| `nexus_actor_mailbox_suspension_resume_count` (counter) | サスペンド解除回数 | `actor_type`, `address` |
| `nexus_actor_mailbox_queue_length` (gauge) | 現在のキュー長 | `queue_kind`, `actor_type`, `address` |

備考:
- `queue_kind` は必須ラベル。Actor 側は `actor_type`、Remote は `address` を基準に見るケースが多い。
- `MAILBOX_QUEUE_SNAPSHOT_INTERVAL` をパネルの説明に明記し、間引き比率を利用者が把握できるようにする。

## 3. Grafana パネル案
1. **Mailbox Queue Percentiles**
   - クエリ: `rate(nexus_actor_mailbox_queue_dwell_percentile_seconds{queue_kind="user", percentile="p95"}[5m])` など。
   - 可視化: 時系列折れ線 (p50/p95/p99)。
   - 備考: snapshot interval が大きい場合は 5〜10 分の移動平均を推奨。

2. **Queue Length Heatmap**
   - クエリ: `MailboxSyncHandle` から収集した現在値を Exporter で Gauge 化 (`nexus_actor_mailbox_queue_length` 仮称) → heatmap。
   - 実装 TODO: 現在はカウンタのみのため、Gauge 出力を追加するか exporter 側で加工する。

3. **Suspension Overview**
   - `nexus_actor_mailbox_suspension_state` の時系列 + `nexus_actor_mailbox_suspension_resume_count` の rate。
   - 状態が 1 の期間を塗りつぶし表示し、アラート発火と連動させる。

4. **Latency vs Snapshot Interval**
   - クエリ: ベンチ／本番で `queue_latency_snapshot_interval` をラベルとして追加 (例: exporter に `snapshot_interval` を新規ラベルとして埋め込む)。
   - TODO: exporter 実装が未対応のため、将来拡張として記録。

## 4. アラート案 (暫定)
| 条件 | レベル | 備考 |
|-------|--------|------|
| `nexus_actor_mailbox_queue_dwell_percentile_seconds{percentile="p95"} > 0.005` (5ms) が 5 分連続 | Warning | バックログ増の早期検知。snapshot interval が大きい場合は 10 分窓を推奨 |
| 同 p95 > 0.020 (20ms) が 3 分連続 | Critical | 直ちに調査 (dispatcher 側負荷を疑う) |
| `nexus_actor_mailbox_suspension_state == 1` が 60 秒以上継続 | Warning | suspend 解除が走らないケース。remote では endpoint_stop を伴う |
| `rate(nexus_actor_mailbox_suspension_resume_count[10m]) > 50` | Warning | 頻繁な suspend/resume でリソース逼迫の可能性 |

通知フロー:
- Warning は Slack の運用チャンネルへ、Critical は PagerDuty へ連携。
- 自動復旧 (enqueue 負荷減) が 10 分以内に見られた場合はアラート自動解消。

## 5. 今後のTODO
- [ ] `nexus_actor_mailbox_queue_length` Gauge 実装 (MailboxSync から exporter へ値を渡す) を検討。
- [ ] snapshot interval をメトリクスのラベルに付与する仕組みを追加し、グラフ上で明示できるようにする。
- [ ] アラートしきい値を本番データで検証し、SLO/SLA と整合が取れているかレビュー。
- [ ] Grafana ダッシュボード JSON を作成し、docs/bench_dashboard_plan.md へリンク。

---
Issue や議論は `#metrics-observability` チャンネルで共有してください。
