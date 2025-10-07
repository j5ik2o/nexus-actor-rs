# Guardian / Supervisor 設計メモ

## 参考資料
- protoactor-go `actor/guardian.go`, `actor/supervision.go`, `actor/actor_context.go`
- 旧 nexus-actor-rs (docs/sources/nexus-actor-rs/modules/actor-std/src/actor/guardian.rs 等)

## 目的
`PriorityEnvelope` に追加した `SystemMessage` / `try_send_system` を生かし、アクター停止・再起動・監視通知といった制御フローを `actor-core` に集約する。std / embedded 側は、実行環境に依存する spawner や timer のラッパーに専念させる。

## ガーディアン構造
### protoactor-go からの着想
- `GuardianProcess` が root / user / system ガーディアンを保持し、`Stop` や `Failure` を子に伝播する。
- SystemMessage は強制的に制御チャネルで処理されるよう mailbox 経由で送出される。

### Rust 側での構成案
- `actor-core` に以下の抽象を用意:
  - `SupervisorTree`: ルート guardian（user/system）と子の `PriorityActorRef<SystemMessage>` を管理する軽量構造。
  - `SupervisorStrategy` (protoactor-go の `SupervisorStrategy` に相当) を trait として定義。`decide` で `SystemMessage::Restart` 等を返す。
  - `GuardianHandle`: 子アクター登録と停止命令を受け取る API。
- `PriorityScheduler` が子アクター登録時に `SupervisorTree` と連携し、`SystemMessage::Watch` 等を生成する。

## SystemMessage フロー
1. 子アクターが生成されると、`ActorContext::spawn_child` が `ChildSpawnSpec` に `SystemMessage::Watch` を追加（protoactor-go `Watch` の送出タイミングを踏襲）。
2. 例外発生時は `SupervisorStrategy::decide` の結果に応じ、`SystemMessage::Failure` や `SystemMessage::Restart` を `try_send_system` で送信。
3. 停止要求 (`SystemMessage::Stop`) は guardian から子へ伝播し、子の mailbox が制御メッセージを最優先で処理。

## API スケッチ
```rust
pub trait SupervisorStrategy<M>: Send + 'static {
  fn decide(&mut self, error: &dyn fmt::Debug) -> SupervisorDirective;
  fn escalation_target(&self) -> Option<PriorityActorRef<SystemMessage, Runtime>>;
}

pub struct Guardian<R>
where
  R: MailboxRuntime,
{
  children: HashMap<ActorId, PriorityActorRef<SystemMessage, R>>,
  strategy: Box<dyn SupervisorStrategy<Message>>, // protoactor-go の OneForOne 相当
}

impl<R> Guardian<R>
where
  R: MailboxRuntime,
{
  pub fn add_child(&mut self, id: ActorId, control_ref: PriorityActorRef<SystemMessage, R>) { ... }
  pub fn stop_child(&mut self, id: ActorId) { control_ref.try_send_system(SystemMessage::Stop); }
  pub fn escalate(&mut self, failure: Failure) { ... }
}
```

## 今後の実装ステップ
1. `actor-core` に ActorId / Failure などの最小構造を導入し、`Guardian` と `SupervisorStrategy` を実装。
2. `PriorityScheduler` が panic を検出した際に `SupervisorStrategy::decide` を呼び出して `SystemMessage` を送出。
3. std / embedded では既存の spawner と統合し、`Guardian` をルートに差し込む。
4. テスト: 子アクターの停止・再起動シナリオ、Watch/Unwatch の伝播、Escalate の確認。

