# Cluster 再構築メモ (2025-09-29)

## 完了事項
- gossip / consensus 系コードを撤去し、`Cluster` を Virtual Actor 前提のシンプルな構成へ再構築。
- `ClusterKind::virtual_actor` と `make_virtual_actor_props` を導入し、Virtual Actor の登録から Props 生成までを統一化。
- `VirtualActorRuntime` を追加し、`respond`/`tell`/`request(_future)`/`forward`/`spawn_child` 等のユーティリティを提供。`Cluster::request_future` も併せて整備し、`ActorFuture::result()` で応答取得できるようにした。
- Rendezvous Hash に基づく `PartitionManager` / `PlacementActor` を実装し、Virtual Actor の所有ノード決定と移譲に対応。
- `DistributedIdentityLookup` を導入し、PartitionManager と連携した `(kind, identity) → PID` 解決の集中管理を可能にした。
- `Cluster::ensure_remote` を追加して Remote 拡張の起動・停止と ActivationHandler 登録をクラスタ本体へ統合。`start_member`/`start_client`/`shutdown` からリモート統合経路を握るようにした。
- InMemory プロバイダーを Rendezvous と結合し、クラスタ参加時に他ノードへトポロジ更新をブロードキャストする流れを整備。
- `cluster/examples/virtual_actor_basic.rs` で Virtual Actor の基本シナリオ、`cluster/examples/provider_in_memory.rs` でプロバイダー連携の確認ができるようにした。
- ClusterProvider の実装ラインナップを整理し、InMemory 以外（例: gRPC/k8s バッキング）へ拡張できる設計指針と TODO を `docs/cluster_provider_overview.md` (2025-09-30 更新) に集約。
- PartitionManager のリバランス挙動を拡張し、トポロジ変化時に新オーナー側で再アクティベーションと `DistributedIdentityLookup` キャッシュ同期を自動化 (2025-09-30)。
- Remote 経由の Activation/Request を検証する統合テストを追加し、ネットワーク越しの PID 解決とメッセージ往復を継続的に確認できる統合テストを整備 (2025-09-30)。
- gRPC Registry サーバー実装と KeepAlive/Heartbeat/TTL 監視を整備し、CI 連携の E2E テストを追加 (2025-09-30)。


## 未完了事項 (MUST 優先)
- なし (2025-09-30 時点)。
