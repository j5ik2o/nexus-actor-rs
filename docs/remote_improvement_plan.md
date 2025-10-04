# remote クレート改善計画 (2025-09-29 時点)

## 区分基準
- **ゴール**: 改善の到達点。
- **進め方**: 実務プロセスとルール。
- **タスク一覧**: フェーズ別の具体的課題。

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

### Phase 1.5: 双方向ストリーム管理

| サブタスク | 状況 | メモ |
|-------------|------|------|
| 1.5-2. backpressure / 所有権モデル決定 | DESIGN IN PROGRESS | `EndpointWriterMailbox` のキュー上限・DeadLetter 方針を策定し、`RingQueue` を固定サイズ化。詳細は `docs/issues/phase1_5_endpoint_stream.md` 参照。 |
| 1.5-3. 再接続・エラー制御ポリシー策定 | TODO | gRPC ストリーム切断時の再試行間隔、最大リトライ回数、回線復旧通知イベントの扱いを決定。protoactor-go の指数バックオフ設定をベースに Rust のタイマーへ落とし込む。 |
| 1.5-4. 統合テストプラン作成 | TODO | `Remote::start` を利用したエンドツーエンドテストのシナリオ（接続確立/切断/再接続/DeadLetter搬送）を整理し、必要なテストフィクスチャとモックを定義する。 |

### BlockList 操作用 API の公開
- **対象**: `remote/src/remote.rs`, `remote/src/block_list.rs`
- **内容**:
  - `Remote::block_system`, `Remote::unblock_system`, `Remote::list_blocked_systems` を追加し、`BlockList` を操作できるようにする。
  - `ConfigOption` に初期ブロックリスト設定を追加。
- **テスト**:
  - BlockList API のユニットテストと、EndpointReader がブロック済みノードを拒否する end-to-end テスト。
- **備考**:
  - 非同期 API なので `Remote` から `BlockList` のメソッドを await しつつラップする。
  - RemoteRuntime からは `BlockListStore` として参照でき、起動直後から初期ブロックが反映される。

### TransportEndpoint 対応と tonic ブリッジ
- **対象**: `remote-core`, `remote-std`, `cluster-std`
- **内容**:
  - `RemoteTransport`／`RemoteRuntime` をコア抽象として再エクスポートし、std/embedded から利用できるよう統合済み。
  - `TonicRemoteTransport` を追加し、`Remote::start` が `TransportListener` と `TransportEndpoint` を経由して起動するよう再構成。
  - `EndpointWriter` が TransportEndpoint を保持し、new transport を通じて gRPC チャネルを確立する構造に変更。
  - `ClusterConfig -> RemoteOptions -> RemoteConfigOption` で transport endpoint を伝播する仕組みを導入。
  - `Remote::runtime()` から `CoreRuntime` と共有するリモートランタイムを初期化し、ブロックリストを `BlockListStore` として公開。
- **テスト**:
  - `remote::tests::test_advertised_address`、`cluster::tests::remote_activation_*` を TransportEndpoint 経路で成功するまで調整済み。
  - `EndpointWriter` ユニットテストでリモート喪失時のエラーを確認。
- **今後のTODO**:
  - TransportEndpoint を使った TLS/認証設定例を追加。
  - `RemoteTransport::connect` エラー時のリトライポリシーを検討。
  - embedded 向け transport 実装（UART/TCP 等）を設計。

### ドキュメント＆サンプル拡充
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
4. Phase 1.5 完了後のフォローアップとして、Phase 2 着手前に TLS / BlockList API 公開など残課題を棚卸しする。

## 関連する Core 側の優先課題

- **ActorContext ロック構造の改善** (`modules/actor-core/src/actor/context/actor_context.rs:112-160`, `:204-237`): `Arc<Mutex<...>>` を保持したまま非同期処理やコールバックを実行しており、メトリクスやユーザーコードから同じコンテキスト API を呼ぶと自己デッドロックを誘発する。ロック解放タイミングの見直しと責務分割が必須。
- **ExtendedPid の ProcessHandle キャッシュ再設計** (`modules/actor-core/src/actor/core/pid.rs:96-114`): `process_handle.lock().await` を保持したまま ProcessRegistry を再帰的に呼び出すため、同一 PID を辿ると再入で固まる。キャッシュ更新フェーズと ProcessRegistry 参照を分離する設計変更が必要。
- **グローバル Extension 登録の同期整理** (`modules/actor-core/src/extensions.rs:32-58`): `Arc<Mutex<dyn Extension>>` と `Synchronized` の多段ロックで Extension 取得→ロック→downcast を行っており、取得中に Extension 側で Context API を呼ぶと競合しやすい。再入を避けるための読み取り専用構造（例: once_cell + RwLock）へ置き換える。
