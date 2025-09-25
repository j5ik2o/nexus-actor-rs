# remoteクレート概況（2025-09-25時点）

## 現状サマリ
- gRPC サーバ起動からアドレス解決、EndpointManager/EndpointReader の起動までを `Remote::start_with_callback` が一括で担い、拡張としてアクターシステムへ登録される流れが固まっている (`remote/src/remote.rs:176`).
- EndpointManager はハートビート監視・再接続ポリシー・統計取得を内包し、接続ごとの状態遷移を追跡できる運用基盤が整備されている (`remote/src/endpoint_manager.rs:215`, `remote/src/endpoint_manager.rs:289`).

## 最近の実装ハイライト
- PubSub/Deliver 系メッセージの `RootSerializable`/`RootSerialized` 実装が揃い、クラスタ経路でのバッチ配信が Rust 版でも完結するようになった (`remote/src/cluster/messages.rs:26`).
- EndpointWriterMailbox が `poll_many` を用いたバッチ取得に復帰し、設定した `batch_size` を正しく消化できる (`remote/src/endpoint_writer_mailbox.rs:77`).
- gRPC 管理 RPC (ListProcesses/GetProcessDiagnostics) が実装され、ノード上のプロセス列挙と簡易診断が可能になった (`remote/src/endpoint_reader.rs:443`).
- BlockList が受信側のサーバ・クライアント両経路で適用され、遮断判定と EndpointManager 連携が動作している (`remote/src/endpoint_reader.rs:87`).
- ConfigOption が再接続バックオフやバックプレッシャ閾値などの運用パラメータをカバーし、Config 経由で細かく調整できる (`remote/src/config_option.rs:5`).

## 未解決の課題・観測事項
- `ResponseStatusCode` が列挙定義のみで、エラー変換や helper が未整備なため呼び出し側での扱いが難しい (`remote/src/response_status_code.rs:1`).
- EndpointSupervisor の Props 構成に TODO が残っており、ディスパッチャ設定などが Go 実装に追随できていない (`remote/src/endpoint_manager.rs:652`).
- gRPC クライアント接続は `Channel::from_shared("http://…")` の固定設定で TLS や DialOptions を注入できず、環境ごとの最適化余地が残る (`remote/src/endpoint_writer.rs:146`).
- EndpointWriter の多くのメッセージで `tracing::info!` が使われており大量メッセージ時にノイズが大きい (`remote/src/endpoint_writer.rs:306`).

## 推奨アクション
1. `ResponseStatusCode` にエラー変換ヘルパや Result ラッパーを追加し、Activator/EndpointWriter からの扱いを簡潔にする。
2. EndpointSupervisor の Props 構成 (ディスパッチャやメールボックス設定) を protoactor-go と突き合わせて埋め、障害復旧フローを仕上げる。
3. 接続設定 API を拡張し、TLS/keepalive/DialOptions などを `ConfigOption` または専用ビルダーから渡せるようにする。
4. ログレベルの見直しやサマリ metrics 化を行い、運用時の I/O 負荷を抑えつつ統計 API との整合を図る。
