# Cluster Provider 概要 (2025-09-30 時点)

## 区分基準
- **API 概要**: `ClusterProvider` が担うコア責務と `ClusterProviderContext` の構造。
- **ラインナップ**: 現行 InMemory と今後追加するプロバイダー候補を MECE に整理。
- **設計指針**: gRPC / Kubernetes バッキングで共通化すべき設計ポリシー。
- **TODO**: 実装着手のための具体的タスクと優先度。

## API 概要
- `cluster/src/provider.rs` で `ClusterProvider` が定義され、`start_member` / `start_client` / `shutdown` に加えて `resolve_partition_manager` を要求。後者でクラスタ内の他ノードが保持する `PartitionManager` を参照できる。
- `ClusterProviderContext` は `cluster_name`・`node_address`・`kinds`・`partition_manager`・`core_runtime` を保持し、`EventStream` を介したトポロジ通知を行う。`provider_context_from_kinds` が `Cluster` から構築し、トポロジ更新時には `TopologyEvent` を publish できる。
- `Cluster::start_member` / `start_client` / `shutdown` はプロバイダーへ委譲しつつ、Virtual Actor ランタイム (`cluster/src/virtual_actor.rs`) と連携する。

## ラインナップ整理

| プロバイダー | ステータス | 主要機能 | 依存先 | 想定ユースケース |
|--------------|------------|----------|--------|--------------------|
| InMemory | 実装済み（開発・テスト用） | ノード・クライアントのインメモリ登録、`PartitionManager` 参照、トポロジブロードキャスト | `tokio::sync::RwLock` のみ | ローカルテスト、単体ノード検証 |
| gRPC Registry | 実装中 | ノード登録を gRPC 経由で共有、`PartitionManager` のリモート解決、イベント通知、Heartbeat/TTL 監視 | gRPC Registry サービス（同梱実装） | 複数ノードを安易に接続するスタンドアロン構成 |
| Kubernetes | 設計着手 | `EndpointSlice`/`Lease` 監視、`PartitionManager` の再付与、ゾーンアフィニティ管理 | Kubernetes API、`kube` クライアント | マネージド Kubernetes 上でのクラスタ展開 |

**InMemory**
- `members`（`HashMap<String, Vec<String>>`）と `clients`（`HashSet<String>`）を保持し、起動時に登録・停止時は現状 no-op。
- `partition_managers` を `Weak<PartitionManager>` としてキャッシュし、`broadcast_topology` で存命なマネージャへ `ClusterTopology` を伝搬。
- `members_snapshot` / `clients_snapshot` / `topology_snapshot` などのデバッグ API を提供し、結合テストで活用。

**gRPC Registry プロバイダー**
- Registry サービスに対し `Join`/`Leave`/`Heartbeat` を gRPC で送出し、他ノードからの `PartitionManager` 参照は registry 経由で ``(cluster_name, node_address)`` をキーに解決。
- トポロジ変更時は registry が Pub/Sub で通知し、ローカル `PartitionManager` が `ClusterTopology` を再計算。
- 連続 Heartbeat を監視し、TTL 超過時はサーバー側でノードを自動除外してフェイルオーバをトリガー。KeepAlive 設定は gRPC チャネル／サーバー双方に適用。
- `TopologyEvent`（`registered` / `snapshot` / `deregistered`）を `EventStream` に流し、`Cluster::ensure_remote` 側で監視ログを出力する。

**Kubernetes プロバイダー（新規）**
- `ClusterProviderContext.node_address` を Pod IP + ポートから生成し、`ClusterKind` 情報は `ConfigMap` または `CustomResource` に反映。
- `PartitionManager` リバランスは `Lease`/`EndpointSlice` のイベントでトリガーし、ゾーン情報（label）を活用した優先度付きアサインを行う。
- 停止時は `Finalizer` によりリソースクリーンアップを保証し、再参加時のリソース再利用を最小化。

## 設計指針（gRPC / Kubernetes 共通）

1. **責務分割**
   - Provider は「ノード登録・解決」のみ担当し、Virtual Actor のアクティベーションは `PartitionManager`/`IdentityLookup` に委譲。
   - トポロジ変更通知を受けた際は `ClusterTopology` を構築し、必ず `PartitionManager::update_topology` を呼び出す。
   - 監視用に `TopologyEvent` を `EventStream` へ publish し、`Cluster::ensure_remote` の購読経路（`topology event observed` ログ）と連携する。

2. **データモデル**
   - 共有キー: `(cluster_name, node_id, advertised_address)` を基礎とし、`kinds` の配列は Provider 側でキャッシュ。
   - `resolve_partition_manager` は provider 側が保持する接続情報（gRPC チャネルまたは Kubernetes オブジェクト）を用い、`Arc<PartitionManager>` を安全に返却。

3. **フォールトトレランス**
   - gRPC: Client-side keepalive と Server-side TTL を設定し、切断時は自動で `ClusterTopology` を無効化。
   - Kubernetes: `Lease` 期限切れでノード離脱を検知し、`PartitionManager` に所有権移譲を要求。

4. **セルフヒーリング TODO**
   - ノード復帰時に `DistributedIdentityLookup` と連携し、キャッシュ再構築を行う API を Provider 層で公開すること。

## TODO（優先度付き）

1. **MUST / Sprint**
   - なし（2025-09-30 時点）。

2. **SHOULD / 次スプリント**
   - Kubernetes プロバイダーの PoC: `EndpointSlice` 監視で `ClusterTopology` を再構築し、`kubectl rollout` と連動した再バランスを確認。
   - `ClusterProviderContext` に `zones: Vec<String>` / `metadata: HashMap<String, String>` を追加し、ゾーンアフィニティ戦略の拡張を可能にする。実装済み InMemory は空配列で対応。

3. **COULD / バックログ**
   - Consul / etcd プラグイン設計検討（gRPC Registry と同じ API を実装したアダプタ）。
   - `broadcast_topology` を `ClusterEvent::TopologyChanged` として `Cluster` へ転送し、監視やメトリクス（Prometheus）へ出力する仕組みを追加。

> 上記 TODO は `docs/worknotes/2025-09-29-cluster-rework.md` の未完了項目と同期済み。gRPC プロバイダーが最優先。
