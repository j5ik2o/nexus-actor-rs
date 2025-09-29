# Cluster Provider 概要

## 目的
`ClusterProvider` はクラスタのメンバーシップ管理を抽象化するトレイトです。protoactor-go と同様に、環境ごとの実装（インメモリ、Consul、Kubernetes など）を差し替えられるよう設計しています。

## トレイト
```rust
#[async_trait]
pub trait ClusterProvider {
  async fn start_member(&self, ctx: &ClusterProviderContext) -> Result<(), ClusterProviderError>;
  async fn start_client(&self, ctx: &ClusterProviderContext) -> Result<(), ClusterProviderError>;
  async fn shutdown(&self, graceful: bool) -> Result<(), ClusterProviderError>;
}
```

`ClusterProviderContext` にはクラスタ名と登録済み Kind の一覧が入ります。`Cluster::start_member` / `start_client` / `shutdown` から呼び出され、`ClusterConfig::with_provider` で差し替えが可能です。

## インメモリ実装
デフォルトでは `InMemoryClusterProvider` が利用されます。メンバー／クライアントを内部の `HashSet` に保持するだけの軽量実装ですが、テストやローカル開発用途に利用できます。

```rust
let provider = Arc::new(InMemoryClusterProvider::new());
let config = ClusterConfig::new("test-cluster").with_provider(provider.clone());
let cluster = Cluster::new(actor_system, config);
cluster.start_member().await?;
```

`snapshot` メソッドで登録状況を検査できます。

## 今後の構想
- Consul / etcd / Kubernetes バッキングの Provider を Rust でも実装可能。
- `ClusterProviderContext` にノード情報やシャードメタデータを追加し、外部ストア連携を強化する。
