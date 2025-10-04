# remote クレート概況 (2025-09-29 時点)

## 区分基準
- **現状サマリ**: 主要コンポーネントの仕組みと確認箇所。
- **最近のハイライト**: 直近で整備された改善点。
- **残課題**: 今後のフォローアップ項目。

## 現状サマリ
- `Remote::start_with_callback`（`remote/src/remote.rs:170-260`）が gRPC サーバの立ち上げ、EndpointSupervisor 起動、ActorSystem 登録までを一括処理。
- `EndpointManager` は Heartbeat 監視と再接続キューを保持し、`endpoint_manager.rs:200-420` で backpressure 設定・DeadLetter 統計を管理。
- `EndpointSupervisor` は `Props::from_async_actor_producer_with_opts` を用いて EndpointWriter/Watcher を生成し、Mailbox シンクを付与（`remote/src/endpoint_supervisor.rs:25-120`）。

## 最近のハイライト
- `ResponseStatusCode` に `ensure_ok` / `as_error` / `ActorPidResponseExt` を追加し、応答ステータスを Result 風に扱えるよう改善（`remote/src/response_status_code.rs`）。
- EndpointWriterMailbox がスナップショット間隔とバッチサイズを Config から取得し、`endpoint_writer_queue_snapshot_interval_samples_updates` テストで回帰を確認。
- gRPC 管理 RPC（ListProcesses / GetProcessDiagnostics）が利用可能で、`remote/src/endpoint_reader.rs:420-520` で診断応答を提供。

## 残課題
- `endpoint_writer_queue_snapshot_interval` は既定値を 32 に更新済み（本番推奨）。staging/dev の推奨値（8/1）の運用ルールを Runbook に追記する。
- BlockList を公開 API（`Remote::block_system` など）から操作できるようにし、送受信経路の拒否制御と DeadLetter 統計連携を整備する。
- TLS / keepalive など Channel 設定を `ConfigOption` 経由で注入する API を拡張する。現在は `Channel::from_shared("http://…")` 固定。
- `EndpointWriter` のログレベルは `tracing::info!` が多く、ハイボリューム環境でノイズとなるためメトリクス化や sample ロギングを検討する。
