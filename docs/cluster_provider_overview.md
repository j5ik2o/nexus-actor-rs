# Cluster Provider 概要 (2025-09-29 時点)

## 区分基準
- **API 概要**: `ClusterProvider` トレイトとコンテキストの仕様。
- **既定実装**: `InMemoryClusterProvider` がどのようにクラスタ状態を管理しているか。
- **拡張余地**: 新規プロバイダーや改善案。

## API 概要
- `cluster/src/provider.rs` で `ClusterProvider` が定義され、`start_member` / `start_client` / `shutdown` に加えて `resolve_partition_manager` を要求。後者でクラスタ内の他ノードが保持する `PartitionManager` を参照できる。
- `ClusterProviderContext` は `cluster_name`・`node_address`・`kinds`・`partition_manager` を保持し、`provider_context_from_kinds`（`cluster/src/provider.rs:118`）で `Cluster` から構築される。
- `Cluster::start_member` / `start_client` / `shutdown` はプロバイダーへ委譲しつつ、Virtual Actor ランタイム (`cluster/src/virtual_actor.rs`) と連携する。

## 既定実装（InMemory）
- `InMemoryClusterProvider` は `members`（`HashMap<String, Vec<String>>`）と `clients`（`HashSet<String>`）を `RwLock` で保持し、起動時に登録／停止時は現状 no-op。
- `partition_managers` は `Weak<PartitionManager>` で管理され、`broadcast_topology` が呼ばれるたびに生存している `PartitionManager` へ最新の `ClusterTopology` を伝搬する。
- `members_snapshot` / `clients_snapshot` / `topology_snapshot` などの補助メソッドで、テストや診断から状態を検査できる。

## 拡張余地
- Consul / etcd / Kubernetes 向けプロバイダーでは `resolve_partition_manager` をクラスタストア経由で実装し、ノード復旧時のトポロジ再配布を自動化できる。
- `ClusterProviderContext` へ gossip 心拍やゾーン情報を追加し、`PartitionManager` のリバランス戦略を拡充する余地がある。
- 監視用途として `broadcast_topology` の発火イベントを `ClusterEvent` として公開し、メトリクスへ転送するタスクが `docs/worknotes/2025-09-29-cluster-rework.md` で検討中。

