# Issue: Phase 3 EndpointWriter 改修・メトリクス整備 (2025-09-29 時点)

## 区分基準
- **現状**: Phase 3 で既に実装済みの内容。
- **残課題**: 追加で対応すべきタスク。
- **テスト/ドキュメント**: 検証状況と資料整備。

## 現状
- `EndpointWriter::initialize` が最大リトライ回数とリトライ間隔を `Config` から取得し、失敗時は `EndpointManager::schedule_reconnect` を呼び出す構造へ移行（`remote/src/endpoint_writer.rs:80-210`）。切断検知時は DeadLetter を発火後に再接続スケジュール。
- `EndpointManager` に `increment_deliver_success` / `increment_deliver_failure` / `increment_reconnect_attempts` を追加し、`EndpointStatisticsSnapshot` で参照可能。
- テスト: `remote/src/endpoint_manager.rs` に再接続イベント・メトリクスのユニットテスト、`remote/src/tests.rs::endpoint_writer_queue_snapshot_interval_samples_updates` でキュー状態の更新を確認。

## 残課題
- 成功／失敗メトリクスを OpenTelemetry Exporter (`remote/src/metrics.rs`) へ配信し、Grafana テンプレートに取り込む実装が未完。
- 再接続サンプル（`remote/examples/reconnect.rs` 相当）が存在せず、ドキュメントから手順を辿れない。サンプル追加と取り扱い説明が必要。
- `Config` の再接続パラメータを README/Docs へ明記し、推奨設定を共有する（現在はデフォルト値のみ記述）。

## テスト/ドキュメント
- 結合テストでの切断→再接続シナリオは `remote/src/tests.rs::endpoint_writer_reconnects_after_disconnect` でカバー済み。高負荷シナリオ（大量連続 Deliver）は未整備。
- ドキュメント: `docs/remote_improvement_plan.md` に本 Issue を紐付け済み。`docs/remote_overview.md` へメトリクス公開状況を追記済み。残りは BlockList/API と合わせたガイド作成が必要。

