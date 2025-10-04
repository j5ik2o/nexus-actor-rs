# remote-core/std/embedded 設計再定義 (2025-10-03)

## 区分基準
- **抽象**: actor-core の新抽象（Timer/CoreSpawner/CoreScheduler 等）に揃えるリモート層の契約。
- **実装分割**: core/STD/embedded それぞれが担う責務整理。
- **タスク**: MUST/SHOULD を優先度付きで列挙。

## 抽象定義（案）
- `RemoteRuntime` (= 旧 RemoteCore) が保持すべき要素
  - `CoreRuntime` 参照
  - `RemoteTransport` 実装（`connect`/`accept`）
  - `SerializerRegistry` / `BlockListStore` / `MetricsSink`
- `RemoteTransport`
  - `type EndpointHandle`
  - `async fn connect(&self, endpoint: &TransportEndpoint) -> Result<EndpointHandle, TransportError>`
  - `async fn serve(&self, listener: TransportListener) -> Result<(), TransportError>`
  - `fn spawn(&self, future: impl Future<Output = ()> + Send + 'static)` などは `CoreSpawner` を利用
- メトリクス/ロギング抽象は `CoreLogger` へ接続し、core 側で注入。

## 実装分割
- `remote-core`
  - メッセージ型、シリアライザ契約、Transport トレイト、RemoteRuntime ロジック。
  - `no_std + alloc`、`embassy` feature で `nexus-utils-embedded` を利用。
- `remote-std`
  - tonic/gRPC 実装、Tokio 依存、メトリクス送信。
- `remote-embedded`
  - `nexus-utils-embedded` ベースの軽量トランスポート（例: raw TCP/UDP/カスタムリンク）を実装。

## タスク
- MUST-1: `RemoteRuntime` 新 API 草案作成（core 抽象に従い、ActorSystem と CoreRuntime の境界を整理）。
- MUST-2: `RemoteTransport` トレイト定義・ドキュメント化。
- MUST-3: `remote-core` の Cargo から `nexus-remote-std-rs` 依存を外し、コア型を移設。
- SHOULD-1: `remote-embedded` 雛形作成、ベアトランスポートの PoC（適当な channel を使ったテスト）。
- SHOULD-2: `remote-std` 実装を新抽象に追従させるリファクタリング計画を作成。

## 進捗整理（2025-10-04 時点）
- **API 実装**: remote-core で `core_api` を再エクスポートし、remote-std/remote-embedded が同一抽象を直接利用できるよう統合済み。
- **std トランスポート**: `TonicRemoteTransport` を追加し、`Remote::start_with_callback` が `TransportListener` と `TransportEndpoint` を経由して起動する構造に整理。`EndpointWriter` も新トランスポートを用いて gRPC チャネルを確立。
- **設定伝播**: `RemoteConfigOption::with_transport_endpoint` を導入し、`ClusterConfig -> RemoteOptions -> RemoteConfig` の流れで advertised address/host/port を一元的に渡す仕組みを実装。
- **embedded 雛形**: Loopback ベースの `RemoteRuntime` 構成テストを追加し、core 抽象との連携を検証。今後は実デバイス向けトランスポート拡張が課題。
- **Runtime 連携**: `RemoteRuntimeConfig::new` が `CoreRuntime` と `TonicRemoteTransport` を束ねるよう更新し、`Remote::runtime()` からブロックリスト共有と将来の spawn 委譲が行える土台を整備。
- **BlockList 初期化**: `ConfigOption::with_initial_blocked_member` を追加し、Remote 起動時に BlockListStore へ初期値を流し込む経路を整備。

## 次のステップ
- **ドキュメント**: `remote_improvement_plan.md` を更新し、TransportEndpoint ベースの設定手順と tonic ブリッジの要点を明文化する。
- **テスト強化**: EndpointWriter の接続失敗パスや Cluster のリモート回帰シナリオを追加し、TransportEndpoint 経由の安定性を確認する。
- **embedded 拡張検討**: UART/TCP など低層トランスポート案と必要な SerializerRegistry 実装を洗い出し、SHOULD 項目に優先度を付与する。
