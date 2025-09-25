# remote クレート改善計画

## ゴール

- protoactor-go に近い遠隔通信機能セットを Rust 実装でも提供する。
- 既存の基本機能を壊さず、段階的に不足分を補完する。
- 実装ごとにユニットテスト／結合テストを追加し、回帰を防ぐ。

## 進め方

1. **役割別のロードマップ管理**
   - 1つの改善テーマを完了するたびに本計画を更新し、進捗を明示する。
   - 各タスクは「設計 → 実装 → テスト → ドキュメント」の順に進める。

2. **テスト整備の原則**
   - 新規 API には必ずユニットテストを追加。
   - gRPC 経路を触る場合は `Remote::start` を使った結合テストを用意し、`#[tokio::test]` を活用する。
   - 既存の `remote/src/remote/tests.rs` を拡張し、主要シナリオを網羅する。

## 最新進捗（2025-09-24）
## 最新進捗（2025-09-24 更新）

- 再接続ポリシーの基盤整備（EndpointState, Config 拡張）と Backpressure 統計・シグナルの追加を Phase 1.5-1 の成果として反映済み。
- `EndpointWriterMailbox` の固定長キュー化と DeadLetter / 統計連携テスト( `client_connection_backpressure_overflow` )を追加。
- `EndpointThrottledEvent` を EventStream に発火できるようになり、Backpressure 状態を監視可能。
- Remote API に `get_endpoint_statistics` を追加（テスト・診断コードから利用可能）。
- `spawn_remote` / `spawn_remote_named` API を追加し、ResponseStatusCode ベースの Result ハンドリングとエラー時テスト (`spawn_remote_unknown_kind_returns_error`) を整備。

- `feat(remote): ListProcessesとGetProcessDiagnosticsのRPCを実装` により診断系 RPC が Rust 版でも利用可能になった。
- `feat(core): ProcessRegistryにプロセス一覧と取得のAPIを追加` でリモートモジュールが参照する基盤 API が整備された。
- `chore: mcp_serenaツールの設定とregex依存関係を追加` によりドキュメント更新や将来の自動化タスクを支援するツールセットが整備済み。
- Phase 1 の成果物に対するテスト（EndpointReader 分離テスト）が追加され、診断 RPC とクライアント接続ハンドシェイクの回帰リスクを低減している。
- Phase 1.5-1 に該当する `ClientConnection` ストリームの EndpointManager 移譲を実装し、`client_connection_registers_and_receives_disconnect` テストで Connect/Disconnect パスを検証済み（2025-09-24）。

## タスク一覧

### Phase 1: ClientConnection & 診断RPC

| サブタスク | 状況 | メモ |
|-------------|------|------|
| 1-1. `ClientConnection` 分岐の受信処理実装 | DONE | BlockList 判定を通した上で `ConnectResponse` を返すよう実装。現状はハンドシェイクのみ対応し、EndpointManager 連携は別途検討。 |
| 1-2. 双方向ストリームの生成と管理 | TODO | EndpointManager への `ClientConnection` ルーティングと、RemoteWatch/Terminate 配信経路が未確立。Phase 1.5 として扱い、Channel 所有権と backpressure 方針を整理する。 |
| 1-3. 診断 RPC (`list_processes` / `get_process_diagnostics`) 実装 | DONE | ProcessRegistry に列挙 API を追加し、RPC を実装。ActorProcess の死活情報を返すようにした。 |
| 1-4. テスト整備 | DONE | EndpointReader 向けの分離テストを追加し、ClientConnection/診断 RPC/プロセス列挙をカバー。 |
| 1-5. ドキュメント更新 | DONE | 2025-09-24 時点の進捗を反映。本計画で双方向ストリーム対応の作業項目と検討事項を整理。 |

### Phase 1.5: 双方向ストリーム管理

| サブタスク | 状況 | メモ |
|-------------|------|------|
| 1.5-1. EndpointManager 連携設計 | DONE | `EndpointReader` から `EndpointManager` へ `ClientConnection` を移譲する責務分割を実装。`send_to_client` API 追加と登録/解除処理を整備し、`client_connection_registers_and_receives_disconnect` テストで検証済み。Issue: `docs/issues/phase1_5_endpoint_stream.md`。 |
| 1.5-2. backpressure / 所有権モデル決定 | DESIGN IN PROGRESS | `EndpointWriterMailbox` のキュー上限・DeadLetter 方針を策定し、`RingQueue` を固定サイズ化。詳細は `docs/issues/phase1_5_endpoint_stream.md`「Phase 1.5-2 設計ドラフト」参照。 |
| 1.5-3. 再接続・エラー制御ポリシー策定 | TODO | gRPC ストリーム切断時の再試行間隔、最大リトライ回数、回線復旧通知イベントの扱いを決定。protoactor-go の指数バックオフ設定をベースに Rust のタイマーへ落とし込む。 |
| 1.5-4. 統合テストプラン作成 | TODO | `Remote::start` を利用したエンドツーエンドテストのシナリオ（接続確立/切断/再接続/DeadLetter搬送）を整理し、必要なテストフィクスチャとモックを定義する。 |

### 1. ClientConnection ハンドリング実装
- **対象**: `remote/src/endpoint_reader.rs`
- **内容**:
  - `on_connect_request` 内の `ClientConnection` 分岐にて、接続元ノードを EndpointManager に登録し、双方向ストリームを開始する。
  - protoactor-go `remote/endpoint_reader.go` の `handleInboundConnection` を参照し、RemoteWatch/Terminate の配線を合わせる。
- **テスト**:
  - 新しいクライアント接続がブロックリストを通過してイベントストリームに通知されるか確認する統合テスト。
- **備考**:
  - ブロックリスト連携も同時に行う（未許可システム ID は拒否）。

#### 次のステップ（2025-09-24 更新）
- RemoteWatch/Terminate、RemoteDeliver のストリームを `tokio::sync::mpsc` で転送する場合の backpressure 設計を決定する（Phase 1.5-2）。
- 双方向ストリーム再接続時のリトライ／タイムアウト戦略を protoactor-go の `remoteEndpointManager.reconnectToEndpoint` に合わせて検討する（Phase 1.5-3）。
- EndpointWriter 連携テストの設計と、DeadLetter 回避策の評価を進める。

#### 設計観点メモ
- protoactor-go では `endpointManager` が `endpointReader` から受け取った `remoteEndpoint` を `endpointSupervisor` 経由で監視し、`remote.EndpointState` に再接続制御ロジックを集約している。Rust 実装でも `EndpointState` 相当のステートマシンを導入し、責務を `EndpointReader` から切り離す。
- `EndpointManager` が Watch/Terminate のフロー制御を担うことで、Reader 側はハンドシェイク完了とチャネル移譲に集中でき、テスト容易性が向上する。Watch/Terminate の伝搬は `mpsc` ベースで統一し、優先度制御を `select!` で実装する。
- backpressure 設計では `bounded mpsc` と `Semaphore` を併用し、高水位時は `RemoteDeliver` を DeadLetter へフォールバックさせる。これによりクラスタ全体での輻輳を緩和しつつ、監視通知は優先的に配送する。

### 2. リモート診断 RPC の実装
- **対象**: `EndpointReader::list_processes` / `get_process_diagnostics`
- **内容**:
  - core 側の ProcessRegistry から情報を取得し、`ListProcessesResponse` / `GetProcessDiagnosticsResponse` を構築する。
  - protoactor-go `remote/endpoint_reader.go` の `handleListProcesses` などを参照。
- **テスト**:
  - gRPC クライアントをモック化したユニットテスト、もしくはテスト用 client を使った結合テストを追加。
- **備考**:
  - 戻り値のフィールド設計を prost 生成コードに合わせて調整する。

### 3. ConfigOption の拡張
- **対象**: `remote/src/config_option.rs`, `remote/src/config.rs`
- **内容**:
  - TLS、`tonic::transport::ClientTlsConfig`, `ServerTlsConfig` の設定を追加。
  - gRPC の Dial/Call オプション（`tower::timeout`, `tonic::codegen::InterceptedService` 等）を選択できるよう struct を拡張。
- **テスト**:
  - 設定適用のユニットテスト（`ConfigOption::apply` でフィールドが更新されるか）。
- **備考**:
  - protoactor-go の `WithDialOptions` / `WithServerOptions` の最低限に相当する機能を目標とする。

### 4. BlockList 操作用 API の公開
- **対象**: `remote/src/remote.rs`, `remote/src/block_list.rs`
- **内容**:
  - `Remote::block_system`, `Remote::unblock_system`, `Remote::list_blocked_systems` を追加し、`BlockList` を操作できるようにする。
  - `ConfigOption` に初期ブロックリスト設定を追加。
- **テスト**:
  - BlockList API のユニットテストと、EndpointReader がブロック済みノードを拒否する end-to-end テスト。
- **備考**:
  - 非同期 API なので `Remote` から `BlockList` のメソッドを await しつつラップする。

### 5. EndpointWriter の再接続とメトリクス整備
- **対象**: `remote/src/endpoint_writer.rs`
- **内容**:
  - 接続断検知時に指数バックオフで再接続を試みる処理を追加。
  - 送信成功／失敗の統計を `tracing` または `metrics` crate 連携で記録する。
- **テスト**:
  - 接続断シナリオを模したユニットテスト（チャネルを意図的に閉じて再接続が発火するか）。
  - メトリクスが更新されるかの検証（mock exporter を利用）。
- **備考**:
  - protoactor-go の `remoteEndpointWriter` の挙動に合わせて DeadLetter の扱いを整える。

### 6. ドキュメント＆サンプル拡充
- **対象**: `docs/`, `remote/examples/`
- **内容**:
  - 新しい API と設定手順をドキュメントに追記。
  - TLS 接続サンプルや BlockList 制御サンプルを追加。
- **テスト**:
  - 実行サンプル用の `cargo run --example` を CI タスクに組み込み検証。
- **備考**:
  - protoactor-go の README/サンプルに近い情報量を目指す。

## スケジュールイメージ

| フェーズ | 期間目安 | 主なアウトプット |
|----------|----------|--------------------|
| Phase 1  | 1週間    | ClientConnection 実装 + 診断 RPC | 
| Phase 2  | 1週間    | ConfigOption 拡張 + BlockList 公開 API（TLS は当面扱わない） |
| Phase 3  | 1週間    | EndpointWriter 改修 + メトリクス + ドキュメント更新 |

(優先順位や期間はリソースに応じて調整)

## 依存関係・検討事項

- TLS やメトリクス導入に伴う依存 crate のバージョン整合。
- protoactor-go の挙動差異がある場合は互換性要否を確認。
- 共通シリアライザ（proto/json）に追加が必要な場合は `serializer.rs` の拡張も検討。
- EndpointManager が保有する `Endpoint` と gRPC ストリーム間の責務分担を明確化し、loopback メッセージの二重処理を防ぐアーキテクチャ指針をまとめる。

## 次のアクション

1. Phase 1.5-2（backpressure / 所有権モデル）に着手し、`EndpointWriter`／`EndpointManager` のキュー設計と DeadLetter 方針を具体化する。
2. 再接続ポリシー（Phase 1.5-3）の検証用シナリオを設計し、`EndpointWriter` のリトライ挙動をテストで再現する。
3. `Remote::start` を利用した結合テスト環境を整備し、ClientConnection/Echo/Disconnect を含む回帰テストをスイート化する。
4. Phase 1.5 完了レポートをまとめ、Phase 2 へ移行するための残課題（TLS、BlockList API 公開など）を棚卸しする。
