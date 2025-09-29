# Cluster 再構築メモ (2025-09-29)

## 完了事項
- gossip / consensus 系コードを撤去し、`Cluster` を Virtual Actor 前提のシンプルな構成へ再構築。
- `ClusterKind::virtual_actor` と `make_virtual_actor_props` を導入し、Virtual Actor の登録から Props 生成までを統一化。
- `VirtualActorRuntime` を追加し、`respond`/`tell`/`request(_future)`/`forward`/`spawn_child` 等のユーティリティを提供。`Cluster::request_future` も併せて整備し、`ActorFuture::result()` で応答取得できるようにした。
- Cluster テストを Virtual Actor ランタイム API ベースに刷新し、起動済み PID のキャッシュとメッセージ配送、Future 応答を検証。
- `cluster/examples/virtual_actor_basic.rs` を追加し、Virtual Actor の登録と `request_message` / タイムアウト処理の流れを最小構成で確認できるようにした。

## 未完了事項
- (なし) — 今後の着手は必要に応じて追加する。
