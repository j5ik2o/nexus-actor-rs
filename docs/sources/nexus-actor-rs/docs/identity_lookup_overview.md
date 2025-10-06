# IdentityLookup 概要 (2025-09-29 時点)

## 区分基準
- **提供 API**: トレイトとコンテキストの仕様。
- **実装バリエーション**: 既存の Lookup 実装と用途。
- **今後の展開**: 設計上の拡張ポイント。

## 提供 API
- `cluster/src/identity_lookup.rs` で `IdentityLookup` トレイトを定義。`setup` / `shutdown` は既定実装で no-op、`get`・`set`・`remove`・`list` を実装側が担当する。
- `IdentityLookupContext` はクラスタ名と Kind 一覧を保持し、`identity_lookup_context_from_kinds` で `Cluster` 起動時に生成される。
- `IdentityLookupHandle = Arc<dyn IdentityLookup>` を通じてクラスタ本体へ DI される。

## 実装バリエーション
- **InMemoryIdentityLookup**: `DashMap<ClusterIdentity, ExtendedPid>` を保持するローカル専用実装。テストおよび単一ノード検証向けで、`snapshot()` により状態確認が可能。
- **DistributedIdentityLookup**: `PartitionManager` と連携し、`activate` を通じて Virtual Actor を動的に起動。`attach_partition_manager` で `Cluster::ensure_remote` からマネージャを結合し、`entries` キャッシュで PID を記録する。
- いずれも `IdentityLookup` を `Arc` で共有できるため、`Cluster::shutdown` 時に `list()` でアクティベーションを停止する流れは共通。

## 今後の展開
- protoactor-go の `identitylookup/disthash` を移植し、Consistent Hash の分散配置を Rust 実装へ展開する。
- `DistributedIdentityLookup` に `PartitionManager` 側のリバランス通知をフックし、トポロジ変化時にキャッシュを自動無効化する仕組みを追加する。
- 外部ストア（etcd / Consul）を利用した永続化 Lookup をモジュールとして追加し、ノード再起動時の PID 復元を担保する。

