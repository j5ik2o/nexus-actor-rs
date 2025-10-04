# Issue: Phase 1.5 双方向ストリーム管理 (2025-09-29 時点)

## 区分基準
- **設計結果**: Phase 1.5 で確定し実装された内容。
- **残課題**: 追加検証や改善が必要なポイント。
- **参照**: 関連コードとドキュメント。

## 設計結果
- `EndpointManager` が `EndpointStatistics`・`EndpointStateHandle` を保持し、backpressure/DeadLetter/再接続を集中管理（`remote/src/endpoint_manager.rs:200-420`）。`schedule_reconnect` は指数バックオフ政策を `EndpointState` から取得しつつ、メトリクス `EndpointThrottledEvent` を発火。
- `EndpointReader` はハンドシェイク後に `register_client_connection` を通じて接続を `EndpointManager` へ移譲。`ClientResponseSender` は DashMap で追跡し、双方向ストリームを EndpointManager 配下で制御。
- `EndpointWriterMailbox` は固定サイズリングキューと snapshot 間引きを備え、容量オーバー時に `DeadLetterEvent` を EventStream へ発行。Queue 状態は `record_queue_state` でメトリクス化。
- `EndpointState` が heartbeat・再接続ポリシーを保持し、`HeartbeatConfig`・`ReconnectPolicy` により指数バックオフと最大リトライを調整。ユニットテスト `schedule_reconnect_successfully_restores_connection` でフローを検証。
- テスト: `remote/src/tests.rs` に backpressure・再接続・RemoteDeliver/Terminate/Watch の統合シナリオを追加し、DeadLetter 件数やイベント発火を確認。

## 残課題
- BlockList API 公開 (`Remote::block_system` など) は未着手。`docs/issues/phase2_config_blocklist.md` へ引き継ぎ済み。
- TLS/keepalive など gRPC DialOptions は固定値のまま。Config で差し替え可能にする必要あり。
- Metrics: Backpressure イベントを OpenTelemetry Exporter へ流す Hook（`remote/src/metrics.rs`）の拡張と、Grafana ダッシュボードのテンプレート化が未完。
- 大規模負荷テスト（数千リモート PID の連続 Deliver）の自動化が未整備。`bench/remote` 配下に Criterion ベンチを追加する。

## 参照
- 実装: `remote/src/endpoint_manager.rs`, `remote/src/endpoint_writer.rs`, `remote/src/endpoint_reader.rs`
- 仕様メモ: `docs/remote_improvement_plan.md`
- protoactor-go 参照: `docs/sources/protoactor-go/remote/endpoint_manager.go`

