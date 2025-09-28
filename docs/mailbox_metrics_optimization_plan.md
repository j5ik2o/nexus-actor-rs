# Mailbox Metrics Optimization Plan (2025-09-28)

## 背景
- フェーズ3で同期メトリクス（滞留ヒストグラム、サスペンド指標）を導入した結果、ホットパスでの計測コストが顕在化した。
- スナップショット間引き（`queue_latency_snapshot_interval`）で大幅な改善を得たが、依然として同期化前ピークよりスループットが低い。
- メトリクスを可視化・アラート化するためには、ラベル設計としきい値設定を確立する必要がある。

## ゴール
1. Mailbox ホットパスにおけるメトリクス更新の追加オーバーヘッドを最小化する。
2. 収集したメトリクスをダッシュボード（Grafana など）へ取り込み、運用で活用できる形にする。
3. suspend/resume を含む重要指標にアラートしきい値を設定し、SLO/SLA を補完する。

## TODO
- [x] **Analyze**: `queue_latency_snapshot_interval` の効果と最適値を複数シナリオ（低負荷・高負荷）でベンチ計測し、推奨設定をまとめる。
- [x] **Design**: メトリクス更新のバッチ化／サンプリング案（例: N 回に 1 回の Atomic 更新、Relaxed オーダー利用時の影響分析）を PoC として実装・比較する。
  - DefaultMailbox に `should_emit_latency_update` を導入し、`MessageInvoker::record_mailbox_queue_latency` をインターバル処理へ間引き。初回メッセージは強制的にサンプル採取し、以降は `queue_latency_snapshot_interval` ごとに更新する。(2025-09-28)
- [x] **Design**: メトリクスシンク未設定時に `queue_latency_metrics()` でのスナップショット複製をスキップするロジックを検討し、実装方針を決定する。`MessageInvokerHandle::new_with_metrics` で購読有無を宣言し、DefaultMailbox がホットパス処理を自動スキップする構成を採用 (2025-09-28)。
- [x] **Implement**: DefaultMailbox へ購読ゲーティングと `QueueLatencyTracker` 再配置を適用し、`record_queue_*` から `Mutex` 取得を排除。`test_default_mailbox_*latency_metrics*` で回帰テストを追加 (2025-09-28)。
- [x] **Implement**: EndpointWriterMailbox にも同等のゲーティングを適用し、remote 経路のメトリクス負荷軽減とダッシュボード反映を確認する。`ConfigOption::with_endpoint_writer_queue_snapshot_interval` でサンプリング間隔を指定できるようにし、デフォルトは 1 (従来挙動) / サンプリング時は backlog 開始・0 件・容量閾値・指定間隔で記録されるよう更新 (2025-09-28)。`endpoint_writer_queue_snapshot_interval_samples_updates` テストを追加し、ゲーティング動作とドレイン後の 0 件報告を検証。
- [x] **Implement**: DefaultMailbox の queue length Gauge を enqueue/dequeue 双方で間引き更新し、バックログ増減を OTEL へ即時反映する。`queue_latency_snapshot_interval` に連動した閾値と小規模キュー向けの優先更新を導入し、`test_default_mailbox_emits_queue_length_samples` で回帰確認 (2025-09-28)。
- [x] **Dashboard**: `MailboxMetricsCollector` を追加し、バックグラウンドで `MailboxSyncHandle` からキュー長・パーセンタイルを収集。
  - `nexus_actor_mailbox_queue_length` / `..._queue_dwell_percentile_seconds` に `pid` ラベルを付与し、`queue_kind`・`percentile` 軸の Grafana heatmap/トレンドを作成。
  - 収集間隔は `ConfigOption::with_mailbox_metrics_poll_interval` (既定 250ms) で調整可能。テンプレートは docs/mailbox_dashboard_design.md に反映済み。
- [x] **Alerting**: Collector 出力を基準に SLO しきい値と運用フローを確定。
  - `nexus_actor_mailbox_queue_dwell_percentile_seconds{percentile="p95"}` > 5ms で Warning、> 20ms で Critical。
  - `nexus_actor_mailbox_queue_length{queue_kind="user"}` が 1024 超を 30 秒継続したら Warning、4096 超で Critical。
  - `nexus_actor_mailbox_suspension_state` が 5 秒以上 1.0 維持なら Warning。PagerDuty runbook を docs/mailbox_dashboard_design.md に追記済み。
- [x] **Docs**: `docs/mailbox_sync_transition_plan.md` と `docs/core_optimization_plan.md` に成果を反映し、継続的にメトリクス運用状況を更新する。EndpointWriterMailbox の snapshot 間引きと設定/API を両ドキュメントへ追記 (2025-09-28)。

## フォローアップタスク (2025-09-28 更新)

### MUST
- [ ] **MUST** デフォルト snapshot 間隔とメトリクス設定の確定（目標日: 2025-10-03）
  - 状態: ベンチ結果は揃ったが設定値が環境ごとにばらついている。
  - 推奨対応: (1) 本番デフォルトを `queue_latency_snapshot_interval = 64` に固定し、staging と開発には 8/1 を割り当てる。(2) `ConfigOption::with_endpoint_writer_queue_snapshot_interval` の既定値を 8 → 32 へ更新し、remote 経路の Update を 2025-09-30 までに実装。(3) `mailbox_metrics_poll_interval` の標準値 250ms を維持しつつ、SRE Runbook に運用フローを追記。
  - 成果物: Config PR、docs/mailbox_dashboard_design.md と Runbook の更新、設定テンプレート。
- [ ] **MUST** メトリクストレース整流化の自動テスト追加（目標日: 2025-10-06）
  - 状態: 単体テストは揃っているが、サンプリングロジックの回帰を検出できる CI が未整備。
  - 推奨対応: (1) `core/tests/mailbox_metrics_trace.rs` を追加し、`queue_latency_snapshot_interval` 変化時の `nexus_actor_mailbox_queue_dwell_percentile_seconds` を Golden ファイルで検証。(2) remote integration テストにメトリクス購読有無を切り替えるシナリオを追加。(3) GitHub Actions nightly でメトリクス差分を Slack 通知するジョブを登録。
  - 成果物: 新規テストコード、CI 設定、Slack 通知テンプレート。

## MUST優先アクション詳細 (2025-09-28 更新)
| タスクID | 目的 | 推奨オーナー | 完了条件 | 推奨トラック |
| --- | --- | --- | --- | --- |
| MUST-設定 | スナップショット間隔と購読設定を統一し、protoactor-go の実績値を根拠に採用 | core/metrics チーム (主担当: R. Nakamura) | Config PR マージ、Runbook v2 公開、`queue_latency_snapshot_interval` 64/8/1 の設定ガイド配布 | 1) protoactor-go/actor/mailbox/default_mailbox.go の `mailboxInstrumentation` を参照し、Rust 実装との差分を表化 2) `ConfigOption` 既定値変更の RFC コメント取得 3) Grafana テンプレートに環境別しきい値をプリセット |
| MUST-テスト | メトリクスサンプリングの回帰検出を自動化し、CI/Nightly 両方で可視化 | core+SRE 共同 (主担当: M. Suzuki) | `core/tests/mailbox_metrics_trace.rs` 通過、remote integration 追加シナリオ成功、Slack 通知ジョブ稼働 | 1) protoactor-go の `metrics/mailbox/mailbox_metrics_test.go` の assertion を Rust へ焼き直し 2) Golden ファイルを `tests/data/mailbox_metrics_trace/` に配置 3) GitHub Actions nightly ワークフローに差分比較と通知ステップを追記 |

## 60分自動実行サイクル (メトリクス最適化)
1. **0-15分**: `queue_latency_snapshot_interval` の環境別デフォルトを Config PR 用テンプレートに記載し、protoactor-go 由来の閾値根拠をコメントへ引用。
2. **15-30分**: `core/tests/mailbox_metrics_trace.rs` のスケルトンと Golden データ生成スクリプト (`scripts/gen_mailbox_metrics_trace.rs`) を作成し、CI で再利用できるよう整備。
3. **30-45分**: remote integration テストでメトリクス購読フラグをトグルするケースを追加し、`cargo test -p remote -- mailbox_metrics` で局所実行して期待値を確認。
4. **45-60分**: GitHub Actions nightly のテンプレート (`.github/workflows/mailbox-metrics-nightly.yml`) 草案を起こし、Slack 通知メッセージに p95/p99 グラフのリンクとトラブルシュート手順を記載。最後に Runbook の更新項目を洗い出す。

### SHOULD
- [ ] **SHOULD** メトリクス Collector の backoff アルゴリズム最適化（目標日: 2025-10-10）
  - 状態: 250ms ポーリングで安定しているが、低負荷クラスタでの CPU オーバーヘッドを更に削減したい。
  - 推奨対応: 収集結果が一定閾値以下の場合にポーリング間隔を最大 1s まで指数バックオフする実装を導入し、protoactor-go の `metrics/prometheus_mailbox` の backoff ロジックを参考に Rust へ移植する。
- [ ] **SHOULD** Grafana ダッシュボード v2 の公開と教育セッション実施（目標日: 2025-10-11）
  - 状態: Heatmap テンプレートは存在するが利用チームが限定的。
  - 推奨対応: ダッシュボード v2 に `queue_kind` 別の比較パネルを追加し、SRE/remote/core 連携で 45 分のハンズオンを開催。録画とクイックスタート資料を共有する。

### MAY
- [ ] **MAY** ヒストグラム集計のバッチフラッシュ実装（目標日: 2025-10-18）
  - 状態: `should_emit_latency_update` で間引きは出来ているが、更なるコスト削減余地あり。
  - 推奨対応: `QueueLatencyTracker` にリングバッファを追加し、64 件単位で `Histogram::record_many` を呼ぶバッチ機構を導入。ベンチ結果を docs/mailbox_benchmark_baseline.md に反映。

## 検証計画
1. `cargo bench --bench mailbox_throughput` でスナップショット間隔や Atomic 更新方式を変更した際のスループットを継続計測する。
2. `bench/benches/dispatcher_queue_latency.rs` にも新しい間引き設定を適用し、Dispatcher 経路への副作用を確認する。
3. Suspended 状態の頻度が高いケース（意図的に Suspend/Resume を混在させた統合テスト）で、メトリクスが正しくサンプリングされることを確認する。
4. `core/src/actor/dispatch/mailbox/tests.rs` の `test_default_mailbox_*latency_metrics*` を通じて、購読フラグ有無でメトリクス呼び出しが切り替わることを継続確認する。

## リスクと緩和策
- **メトリクス精度の低下**: サンプリング頻度を下げると短期間の異常を検知できない可能性がある → `queue_latency_snapshot_interval` の上限値とダッシュボード表示で補完。
- **Atomic オーダー変更による可視化の不整合**: `Relaxed` やバッチ更新を導入することで可視化値に遅延が生じる → アラート用メトリクスには現在の Strict オーダーを保持するなど二本立てを検討。
- **コード複雑化**: 最適化ロジックが複雑になるとメンテナンス性が落ちる → feature flag や明確な設定項目として切り出し、デフォルトを安全側に保つ。

## ステークホルダー連携
- **core チーム**: DefaultMailbox の最適化 PoC を主導し、進捗とベンチ結果を週次で共有。
- **remote チーム**: EndpointWriterMailbox の変更点をレビューし、リモート通信への影響やアラート活用シナリオを確認。
- **SRE/Observability チーム**: ダッシュボード案とアラートしきい値を擦り合わせ、システム全体のモニタリング計画に統合する。

- 2025-09-28: スナップショット間隔制御を導入し、`mailbox_throughput` ベンチで 4x 以上の改善を確認。今後の最適化項目を本ドキュメントに整理。
- 2025-09-28: `queue_latency_snapshot_interval` = 1/8/64 で再計測（100 件: 613µs→217µs→145µs、1000 件: 6.14ms→2.14ms→1.07ms）。snap64 が最も高スループット、snap8 は妥協案として有効。
- 2025-09-28: `MessageInvokerHandle::new_with_metrics` と DefaultMailbox のメトリクスゲーティングを実装し、シンク未設定時のホットパス計測コストを削減。新規ユニットテストで購読の有無をカバー。
- 2025-09-28: `DefaultMailbox::should_emit_latency_update` を追加し、キュー滞留メトリクスを `queue_latency_snapshot_interval` 単位でまとめて配信。ベンチホットパスの `record_mailbox_queue_latency` 呼び出しを約 `1/interval` に間引き、`test_default_mailbox_emits_latency_metrics_with_interest` でスナップショット間隔 1 のケースを検証。
- 2025-09-28: EndpointWriterMailbox に queue snapshot 間引き設定 (`endpoint_writer_queue_snapshot_interval`) を導入し、初回積み上がり・容量閾値・カウンタ到達・ドレイン完了で統計を更新するよう変更。ゲーティングを `endpoint_writer_queue_snapshot_interval_samples_updates` で検証。
- 2025-09-28: DefaultMailbox の queue length Gauge を enqueue/dequeue で即時サンプリングし、小規模キューは逐次・大規模キューは `queue_latency_snapshot_interval` 境界で更新するよう調整。`test_default_mailbox_emits_queue_length_samples` で 1/2/0 のサンプル記録を確認。
- 2025-09-28: バックグラウンド収集用 `MailboxMetricsCollector` を実装し、`MailboxSyncHandle` からキュー長・パーセンタイル・サスペンド状態を 250ms 間隔でサンプリング。`ConfigOption::with_mailbox_metrics_poll_interval` を追加し、`test_mailbox_metrics_collector_emits_queue_length_with_pid` で回帰確認。
- （以降の更新はここに追記）

## 推奨設定 (2025-09-28 決定)
- 本番デフォルト: `queue_latency_snapshot_interval = 64`。canary/staging は 8、開発/デバッグは 1 を使用して極端なレイテンシを可視化する。
- remote 経路: `endpoint_writer_queue_snapshot_interval = 32` を新デフォルトとし、backpressure 監視を優先するクラスタのみ 8 へ引き下げる。
- Collector: `mailbox_metrics_poll_interval = 250ms` を標準とし、Heatmap に変化がない場合は backoff 設定で 500ms → 1s へ段階的に伸長する (フォローアップタスク参照)。
- アラート: `nexus_actor_mailbox_queue_dwell_percentile_seconds{percentile="p95"}` しきい値を 5ms / 20ms で維持しつつ、Staging は 3ms / 12ms にチューニングして早期検出を狙う。
