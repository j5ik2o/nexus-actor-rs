# Virtual Actor Runtime ガイド (2025-09-29 時点)

## 区分基準
- **概要**: API と全体像。
- **実装フロー**: 時系列での利用手順。
- **ベストプラクティス**: 運用の指針。


## 概要
Virtual Actor は `ClusterKind::virtual_actor` を通じて登録し、`VirtualActorRuntime` を介してメッセージの送受信や子アクター管理を行います。本ドキュメントは実装時に必要となる主要 API と利用パターンを整理したものです。

## 基本構造
- `VirtualActorContext`
  - `identity()` : `ClusterIdentity` へのアクセス。
  - `actor_system()` : `ActorSystem` の参照を取得。
  - `cluster()` : `Cluster` ハンドルを取得し、他の Virtual Actor へのアクセスに利用。
- `VirtualActorRuntime`
  - `respond` / `tell` / `request` / `request_future` / `forward`
  - `spawn_child` / `spawn_child_named`
  - `watch` / `unwatch`
  - `reenter_after` : 非同期処理完了後の継続を登録。

## 実装フロー (MECE)
1. **起動** : `activate` で `VirtualActorContext` を受け取り、必要な初期化を実行。
2. **メッセージ処理** : `handle` に渡される `MessageHandle` をパターンマッチし、`VirtualActorRuntime` を利用して応答や後続アクションを実行。
3. **停止** : `deactivate` でリソース解放や監視解除を行う。
4. **タイムアウト** : `ClusterConfig::request_timeout()` が既定値（デフォルト 5 秒）を提供する。`Cluster::request_message` / `request_message_with_timeout` を活用することで、Remote 経路と同じポリシーで応答待機が可能。

## ランタイム API チートシート
| 操作            | 説明                                       |
|-----------------|--------------------------------------------|
| `respond(msg)`  | 現在の Sender に対して応答を返す。         |
| `tell(pid, msg)`| 任意 PID へワンウェイメッセージを送信。      |
| `request(pid, msg)` | 問い合わせを送り、Sender を維持。        |
| `request_future(pid, msg, timeout)` | `ActorFuture` を取得し、応答を待つ。 |
| `forward(pid)`  | 受信中メッセージを別 PID へ転送。           |
| `spawn_child(props)` | 匿名子アクターを生成。                 |
| `spawn_child_named(props, name)` | 指定名で子アクターを生成。 |
| `watch(pid)` / `unwatch(pid)` | 監視の開始／解除。             |
| `reenter_after(future, continuer)` | 非同期完了後の継続処理。   |

## サンプルコード抜粋
`cluster/examples/virtual_actor_basic.rs` に実行可能なサンプルを用意しています。以下は抜粋です。

```rust
#[async_trait]
impl VirtualActor for CounterActor {
  async fn activate(&mut self, ctx: &VirtualActorContext) -> Result<(), ActorError> {
    self.identity = Some(ctx.identity().clone());
    Ok(())
  }

  async fn handle(&mut self, message: MessageHandle, runtime: VirtualActorRuntime<'_>) -> Result<(), ActorError> {
    if let Some(increment) = message.as_typed::<Increment>() {
      self.value += increment.0;
      runtime.respond(CurrentValue(self.value)).await;
      return Ok(());
    }

    if let Some(spawn) = message.as_typed::<SpawnWorker>() {
      let worker_props = spawn.props().clone();
      runtime.spawn_child(worker_props).await;
      return Ok(());
    }

    Err(ActorError::of_receive_error(ErrorReason::from("unsupported message")))
  }
}
```

## ベストプラクティス
- `request_future` を利用する際は、`ActorFuture::result()` を待機し、タイムアウト値を必ず設定する。
- 監視開始 (`watch`) 後は停止時に `unwatch` を実行してリークを防ぐ。
- 子アクター生成時は名前重複を避けるため、必要に応じて `spawn_child_named` の結果を検証する。

## 今後の改善案
- `VirtualActorRuntime` にメトリクス記録ヘルパーを追加し、`reenter_after` と連携した測定を容易にする。
- `Cluster` 側の `request_future` に共通タイムアウトポリシーを適用し、Remote との整合性を明確化する。
