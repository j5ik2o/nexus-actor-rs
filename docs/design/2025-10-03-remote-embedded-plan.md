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
