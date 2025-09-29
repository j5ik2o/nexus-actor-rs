# IdentityLookup 概要

`IdentityLookup` は Virtual Actor の `(kind, identity) → PID` 解決をクラスタ全体で扱うためのレイヤーです。protoactor-go の `cluster/identity_lookup.go` と同じ役割を担います。

## インターフェース
```rust
#[async_trait]
pub trait IdentityLookup {
  async fn setup(&self, ctx: &IdentityLookupContext) -> Result<(), IdentityLookupError>;
  async fn shutdown(&self) -> Result<(), IdentityLookupError>;
  async fn get(&self, identity: &ClusterIdentity) -> Result<Option<ExtendedPid>, IdentityLookupError>;
  async fn set(&self, identity: ClusterIdentity, pid: ExtendedPid) -> Result<(), IdentityLookupError>;
  async fn remove(&self, identity: &ClusterIdentity) -> Result<(), IdentityLookupError>;
  async fn list(&self) -> Result<Vec<ExtendedPid>, IdentityLookupError>;
}
```
`IdentityLookupContext` にはクラスタ名と登録済み Kind の一覧が入ります。

## InMemoryIdentityLookup
現在はローカルノードの `DashMap` を利用した InMemory 実装を提供しています。`Cluster::get` / `request_message` / `request_future` はこの Lookup 経由で PID を解決し、`Cluster::shutdown` 時に `list()` を使ってアクティベーションを停止します。

## 今後の展開
- protoactor-go の `identitylookup/disthash` を移植し、Consistent Hash に基づいた分散配置を実現する。
- 外部ストア（etcd/Consul など）と連携する StorageLookup や SpawnLock を導入し、シャード再配置・ノード離脱時の動作を強化する。
- Provider と IdentityLookup を組み合わせ、Akka/Pekko の Cluster Sharding 相当のワークロードへ拡張する。
