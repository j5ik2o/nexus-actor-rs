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

## タスク一覧

### Phase 1: ClientConnection & 診断RPC

| サブタスク | 状況 | メモ |
|-------------|------|------|
| 1-1. `ClientConnection` 分岐の受信処理実装 | DONE | BlockList 判定を通した上で `ConnectResponse` を返すよう実装。現状はハンドシェイクのみ対応し、EndpointManager 連携は別途検討。 |
| 1-2. 双方向ストリームの生成と管理 | TODO | `ClientConnection` の受信ストリームを EndpointManager 側で活用する仕組みは未着手。Phase 1.5 として扱い、要件を再整理する。 |
| 1-3. 診断 RPC (`list_processes` / `get_process_diagnostics`) 実装 | DONE | ProcessRegistry に列挙 API を追加し、RPC を実装。ActorProcess の死活情報を返すようにした。 |
| 1-4. テスト整備 | DONE | EndpointReader 向けの分離テストを追加し、ClientConnection/診断 RPC/プロセス列挙をカバー。 |
| 1-5. ドキュメント更新 | WIP | 本計画を更新済み。`1-2` の対応方針固まり次第追記予定。 |

### 1. ClientConnection ハンドリング実装
- **対象**: `remote/src/endpoint_reader.rs`
- **内容**:
  - `on_connect_request` 内の `ClientConnection` 分岐にて、接続元ノードを EndpointManager に登録し、双方向ストリームを開始する。
  - protoactor-go `remote/endpoint_reader.go` の `handleInboundConnection` を参照し、RemoteWatch/Terminate の配線を合わせる。
- **テスト**:
  - 新しいクライアント接続がブロックリストを通過してイベントストリームに通知されるか確認する統合テスト。
- **備考**:
  - ブロックリスト連携も同時に行う（未許可システム ID は拒否）。

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
| Phase 2  | 1週間    | ConfigOption/TLS 対応 + BlockList 公開 API |
| Phase 3  | 1週間    | EndpointWriter 改修 + メトリクス + ドキュメント更新 |

(優先順位や期間はリソースに応じて調整)

## 依存関係・検討事項

- TLS やメトリクス導入に伴う依存 crate のバージョン整合。
- protoactor-go の挙動差異がある場合は互換性要否を確認。
- 共通シリアライザ（proto/json）に追加が必要な場合は `serializer.rs` の拡張も検討。

## 次のアクション

1. Phase 1 の詳細設計を Issue 化してレビューを受ける。
2. `remote` クレート用の追加テスト環境（統合テスト用グローバル fixture）を整備する。
3. 設計に従って実装を開始し、本計画を随時更新。
