# Typed Actor 設計メモ

## 参考にした実装
- protoactor-go [`actor/props.go`, `actor/typed/behavior.go`]
- Akka Typed (`Behavior`, `Behaviors.receive`, `Behaviors.setup` など)
- 旧 nexus-actor-rs の `typed` API (docs/sources/nexus-actor-rs/modules/actor-std/...)

## 目標
- ユーザーが定義するアクターは型付きメッセージで安全に開発できる。
- ランタイム内部（guardian/scheduler/mailbox）は untyped を維持し、型アダプタで橋渡しする。
- Behaviors DSL により、振る舞いを純粋関数で記述できる（Akka Typed の思想）。
- `map_system` クロージャで `SystemMessage` をユーザー型へ射影し、guardian/scheduler の untyped な制御フローと型付き DSL の整合性を保つ。

## コア API の案

```rust
pub enum Behavior<M, S> {
  Stateless {
    handler: fn(&mut TypedContext<M>, M) -> Behavior<M, S>,
  },
  Stateful {
    state: S,
    handler: fn(&mut TypedContext<M>, &mut S, M) -> Behavior<M, S>,
  },
  Stopped,
}

pub struct TypedContext<'a, M> {
  untyped: &'a mut ActorContext<'a, UntypedMessage, UntypedRuntime, dyn Supervisor<UntypedMessage>>,
  map_fn: &'a dyn Fn(M) -> UntypedMessage,
}

pub struct TypedActorRef<M> {
  inner: PriorityActorRef<UntypedMessage, UntypedRuntime>,
  map_fn: Arc<dyn Fn(M) -> UntypedMessage + Send + Sync>,
}
```

- `Behavior::receive(|ctx, msg| {...})` で Stateless を構築。
- `Behavior::stateful(initial_state, |ctx, state, msg| {...})` で Stateful。
- `TypedContext` が `spawn_child::<Child>` や `ask` 等の型付き API を提供。
- SystemMessage は `TypedContext::on_system_message` で `Behavior::system` ハンドラへ流す。

## SystemMessage 対応
- `PriorityActorRef::try_send_system` を型アダプタから呼ぶ（`map_fn(SystemMessage::Stop)` 等）。
- `Behavior` に `handle_system(&mut ctx, SystemMessage) -> Behavior` を用意し、Supervisor から制御メッセージを受け取れるようにする。
- `ActorContext` へ渡す `map_system: Arc<dyn Fn(SystemMessage) -> M>` を Typed 層が生成し、`Behavior` 側で `SystemMessage` を型安全に処理できるようエントリポイントを設ける。

### map_system の生成ポリシー
- Stateless な typed actor の場合、`map_system` は単純に `TypedSystemEvent::from(system_message)` のような enum 変換を行う。
- Stateful / DSL ベースの actor では、`map_system` が `Behavior` にバインドされた `Arc<dyn Fn(SystemMessage) -> M>` を返す。例えば
  ```rust
  enum MyMsg {
    User(UserMsg),
    System(TypedSystemEvent),
  }

  fn build_map_system() -> Arc<dyn Fn(SystemMessage) -> MyMsg + Send + Sync> {
    Arc::new(|sys| MyMsg::System(TypedSystemEvent::from(sys)))
  }
  ```
- map_system は guardian/scheduler から子アクターへ送る制御メッセージの唯一の経路となるため、Typed 層で生成したクロージャを `Props` 初期化時に `ActorContext::spawn_child` へ渡す。
- 旧 protoactor-go の typed adapter (`ConvertedRecoverable`) を参考に、`Behavior` が変化した場合でも同じ `Arc` を共有できるよう clone 可能な構造を採用する。

## 実装の分割
1. `actor-core`
   - `Behavior<M>` 型（enum／builder）と `TypedContext` の抽象。
   - typed → untyped 変換を担う `TypedMailboxRuntime<M, R>`（`PriorityEnvelope::map` を活用）。
2. `actor-std` / `actor-embedded`
   - 実行環境ごとの typed API（`TypedProps`, `TypedActorSystem`）を提供。
   - `spawn_typed::<MyActor>()` など user-facing API を定義。

## 旧実装から取り込む要素
- `TypedActor` trait のメソッドシグネチャ（start/stop/system メッセージ処理）。
- `ask` / `tell` のシンプルな型付きラッパー。
- `spawn_child` が返す `TypedActorRef` をプロミスベースで扱える仕組み。

## TODO
1. `Behavior<M>` と `TypedContext<M>` の最小実装を `actor-core` に追加。（2025-10-07 完了）
2. typed → untyped アダプタ（`TypedActorAdapter`）を作成し、`map_system` クロージャを生成する API を定義。（2025-10-08 完了）
3. `TypedProps` 初期化時に `map_system` を `ActorContext::spawn_child` へ渡す経路を実装。（2025-10-08 完了）
4. サンプルアクター（stateless/stateful）で単体テストし、SystemMessage が typed 層に伝播する統合テストを整備。

## 実装知見 (2025-10-08)

### TypedActorAdapter の実装
- `TypedActorAdapter<U, R>` 構造体を追加し、`Behavior` と `ActorContext` の橋渡しを実現。
- `handle_user()`: ユーザーメッセージを処理し、内部で `TypedContext` を生成して `Behavior` を駆動。
- `handle_system()`: システムメッセージを処理し、オプショナルな `system_handler` を呼び出し。
- `create_map_system()`: `Arc<dyn Fn(SystemMessage) -> MessageEnvelope<U>>` を返す静的メソッドで、Guardian/Scheduler 統合用の `map_system` クロージャを生成。

### map_system 経路の統合
- `TypedProps::with_behavior_and_system` を `TypedActorAdapter` 使用に改修。
- `Props::new` へ渡すハンドラは Adapter の `handle_user`/`handle_system` を呼ぶシンプルな構造に。
- `map_system` クロージャは `TypedActorAdapter::create_map_system()` が生成し、現状は `MessageEnvelope::System(sys)` にラップ。
- 将来的なユーザー定義 enum への拡張ポイントとして明示的にコメント記載。

### 統合テスト追加
1. **test_typed_actor_handles_system_stop**: `SystemMessage::Stop` を typed 層で処理し、停止フラグをセットできることを検証。
2. **test_typed_actor_handles_watch_unwatch**: `SystemMessage::Watch`/`Unwatch` が `ActorContext::register_watcher`/`unregister_watcher` を通じて反映されることを検証。親アクターが自動的にwatcherとして登録される仕様を考慮し、count-based アサーションを採用。
3. **test_typed_actor_stateful_behavior_with_system_message**: `Behavior::stateless` でstateをキャプチャし、ユーザーメッセージと `SystemMessage::Suspend` の両方で状態を更新できることを検証。

### 技術的課題と解決
- **Lifetime制約**: `TypedActorAdapter::new()` で `Box<dyn for<'ctx> FnMut(...)>` の型推論が曖昧になる問題に遭遇。明示的な完全型アノテーションで解決。
- **テストクロージャの型推論**: テスト用クロージャで lifetime パラメータを明示する必要があり、`move |_: &mut ActorContext<'_, ...>, sys_msg: SystemMessage|` のように記述。
- **親watcher自動登録**: `ActorContext` は親を自動的にwatcherとして登録するため、Watch/Unwatchテストは特定IDの検証ではなく、watcher count の増減を検証する設計に変更。
- **SystemMessage選択**: `SystemMessage::Stop` や `SystemMessage::Restart` はアクター停止や再起動を引き起こすため、テストでは `SystemMessage::Suspend` を使用してアクターを停止させずに検証。
