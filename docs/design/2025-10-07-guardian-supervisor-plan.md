# Guardian / Supervisor 設計メモ

## 参考資料
- protoactor-go `actor/guardian.go`, `actor/supervision.go`, `actor/actor_context.go`
- 旧 nexus-actor-rs (docs/sources/nexus-actor-rs/modules/actor-std/src/actor/guardian.rs 等)

## 目的
`PriorityEnvelope` に追加した `SystemMessage` / `try_send_system` を生かし、アクター停止・再起動・監視通知といった制御フローを `actor-core` に集約する。std / embedded 側は、実行環境に依存する spawner や timer のラッパーに専念させる。2025-10-07 時点で、`Guardian` と `PriorityScheduler` が統合され、panic 検出時に `GuardianStrategy` へ委譲して `SystemMessage` を送出する最小ループが完成した。

## ガーディアン構造
### protoactor-go からの着想
- `GuardianProcess` が root / user / system ガーディアンを保持し、`Stop` や `Failure` を子に伝播する。
- SystemMessage は強制的に制御チャネルで処理されるよう mailbox 経由で送出される。

### Rust 側での現状構成
- `Guardian<M, R, Strat>` が `PriorityActorRef<M, R>` と `map_system: Arc<dyn Fn(SystemMessage) -> M>` を子ごとに保持し、`SystemMessage::Stop` / `Restart` を優先度付きエンベロープへ変換して送信する。
- `ActorContext::spawn_child` / `ChildSpawnSpec` が `map_system` を伝搬し、`PriorityScheduler` が子登録時に Guardian へ渡す。
- panic 検出は `std` feature 時にのみ `catch_unwind` で処理し、no_std では未定義動作を避けるためガーディアン通知を行わず早期復帰する。
- `GuardianStrategy` は protoactor-go の Strategy と同様に `decide` / `before_start` / `after_restart` を提供し、現状は `AlwaysRestart` のみ実装済み。

### 今後の拡張ポイント
- `SystemMessage::Watch` / `Unwatch` など未実装の通知を `map_system` 経由で流せるようにする。
- Escalate の送信先を multi-root（user/system guardian）で切り替えられるよう、`Guardian` にエスカレーション用アクタ参照を保持させる。
- `map_system` を typed 層の DSL が差し替えられるよう、`TypedMailboxAdapter`（仮称）がクロージャ生成を担う。

## SystemMessage フロー（現状）
1. 子アクター生成時、`ActorContext::spawn_child` が `map_system` クロージャを `ChildSpawnSpec` にコピーし、次のディスパッチサイクルで `PriorityScheduler` が Guardian へ登録する。
2. 例外発生時（`std` feature 有効のみ）に `PriorityScheduler::dispatch_envelope` が `catch_unwind` で panic を捕捉し、`Guardian::notify_failure` を呼び出して `SystemMessage::Restart` / `Stop` を制御キューへ投入する。
3. 停止要求 (`SystemMessage::Stop`) は guardian から子に送信され、`PriorityEnvelope` の Control チャネル経由で最優先処理される。
4. no_std 構成では panic を捕捉できないため、今後 `Result` ベースのエラーパスや `SystemMessage::Failure` API を追加する余地がある。

## SystemMessage フロー（今後の計画）
1. `SystemMessage::Watch` / `Unwatch` を導入し、子登録時に Guardian から親へ通知する。
2. Escalate をサポートし、上位 Guardian へ転送する際も `map_system` クロージャを通じてメッセージ型を変換する。
3. Typed 層が `map_system` を生成し、`SystemMessage` をユーザーの DSL に沿った挙動へマッピングできるようインターフェースを整備する。

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
1. `SystemMessage::Watch` / `Unwatch` を追加し、Guardian が親子関係の登録解除を通知可能にする。
2. Escalate ルートを設計し、Guardian が上位監督者へ Failure を伝搬できるよう抽象を拡張する。
3. Typed Actor 層の `map_system` 生成 API を定義し、K型→SystemMessage→K型の変換ポリシーをテストで検証する。
4. no_std 構成向けに panic 以外のエラー経路（`Result` 返却等）を guardian に伝える手段を検討する。
