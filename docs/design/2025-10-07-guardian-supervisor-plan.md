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
- `SystemMessage::Watch` / `Unwatch` は 2025-10-07 実装済み。今後は、親コンテキスト側で通知を受け取り監視テーブルを更新する仕組み（`ActorContext` もしくは `Supervisor` へのフック）を整備する。
- Escalate の送信先を multi-root（user/system guardian）で切り替えられるよう、`Guardian` にエスカレーション用アクタ参照を保持させる。
- `map_system` を typed 層の DSL が差し替えられるよう、`TypedMailboxAdapter`（仮称）がクロージャ生成を担う。

## SystemMessage フロー（現状）
1. 子アクター生成時、`ActorContext::spawn_child` が `map_system` クロージャを `ChildSpawnSpec` にコピーし、次のディスパッチサイクルで `PriorityScheduler` が Guardian へ登録する。
2. 例外発生時（`std` feature 有効のみ）に `PriorityScheduler::dispatch_envelope` が `catch_unwind` で panic を捕捉し、`Guardian::notify_failure` を呼び出して `SystemMessage::Restart` / `Stop` を制御キューへ投入する。
3. 停止要求 (`SystemMessage::Stop`) は guardian から子に送信され、`PriorityEnvelope` の Control チャネル経由で最優先処理される。
4. no_std 構成では panic を捕捉できないため、今後 `Result` ベースのエラーパスや `SystemMessage::Failure` API を追加する余地がある。

## SystemMessage フロー（今後の計画）
1. Guardian が送出した `SystemMessage::Watch` / `Unwatch` を親アクターで経路制御する。具体的には、`ActorContext` が制御メッセージ受信時に `watchers` テーブルへ登録・解除する API を公開する。
2. Escalate をサポートし、上位 Guardian へ転送する際も `map_system` クロージャを通じてメッセージ型を変換する。
3. Typed 層が `map_system` を生成し、`SystemMessage` をユーザーの DSL に沿った挙動へマッピングできるようインターフェースを整備する。

## Watch / Unwatch 親伝播設計草案

### 要件
- Guardian が子 mailbox に投入した `Watch` / `Unwatch` が親アクターにも届き、親側で監視対象リストを管理できること。
- Terminated 通知を受け取った親が `Unwatch` 済みの相手を除外しつつ、必要に応じて追加の SystemMessage（例: `Terminate`）を生成できること。

### 提案する変更
1. `ActorContext` に `handle_system_message`（仮称）を追加し、`PriorityScheduler` が制御メッセージをディスパッチする前に事前処理できるようにする。
2. `ActorContext` 内部に `WatchRegistry`（軽量な `BTreeSet<ActorId>` または `Shared<HashSet<_>>`）を保持し、`Watch` で insert、`Unwatch` で remove を実施する。
3. 親から子への `watch` API を公開する際は、`PriorityActorRef::try_send_system(SystemMessage::Watch)` を包むヘルパーを提供し、Typed 層でも同一ハンドラを利用できるようにする。
4. Terminated 受信時には `WatchRegistry` を参照して該当 watcher へ `SystemMessage::Terminated`（今後追加）を送出する経路を設計する。

### Open Questions
- `Watch` / `Unwatch` は ProtoActor では `PID` を payload に持つ。Rust 実装では `map_system` を通じてユーザー型へ変換する必要があるため、`SystemMessage` に watcher 情報を追加するか、別途 `WatchEvent` 型を envelope で運ぶか検討する。
- 親通知用の SystemMessage を `ActorContext` レベルで消費するか、ユーザーにも透過するか（Akka Typed のように内部処理へ限定するか）を決める必要がある。
- no_std 環境でも `WatchRegistry` を活用できるよう、`Vec` + 線形検索で十分か、あるいは `heapless::IndexSet` 等を利用するかを評価する。

## Failure / Escalate 拡張方針

### 追加するメッセージ
- `SystemMessage::Failure(FailureInfo)`：子アクターから親へ障害情報を伝播。
- `SystemMessage::Escalate(FailureInfo)`：Guardian が上位 Guardian へ障害をエスカレート。
- `FailureInfo` には `actor_id`, `reason`, `restart_stats`, `last_message` のような protoactor-go に準じたフィールドを含める。

### 予定する実装ステップ
1. `FailureInfo` 構造体を `actor-core` に追加し、`PriorityEnvelope<FailureInfo>` を扱えるよう `Element` を実装。
2. `GuardianStrategy::decide` が `SupervisorDirective::Escalate` を返した際、`Guardian` が `SystemMessage::Escalate` を生成し、親 Guardian もしくはルート戦略へ転送する。
3. `Supervisor` 実装に Failure/Escalate を通知するため、`ActorContext` に `notify_failure` フックを追加する。
4. テストシナリオ: (a) 子アクターが panic し再起動。Restart/Stop を確認。(b) 再起動回数上限を超えた場合 Escalate を返す戦略で Failure メッセージが親へ流れることを確認。

### 留意点
- Escalate の送信先は protoactor-go では GuardianProcess（system guardian）固定。Rust 版では `PriorityScheduler` が複数 Guardian を持てるようにし、`GuardianHandle` のような構造を介して上位に通知することを検討。
- FailureInfo の payload サイズが大きくなる場合、`Shared<FailureInfo>` を用いてコピー回数を削減する。
- `map_system` が Failure/Escalate の型変換も担うため、Typed 層では `TypedSystemEvent::Failure(FailureInfo)` のような enum へ写像する実装が必要。

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
1. `ActorContext` に Watch/Unwatch を内部的に処理するフックを導入し、親側の監視レジストリ更新を実装する。
2. `FailureInfo` / `SystemMessage::Failure` / `SystemMessage::Escalate` を追加し、Guardian と SupervisorStrategy 間のエラーフローを整備する。
3. Typed Actor 層の `map_system` 生成 API を定義し、`Watch/Unwatch/Failure` を型安全に扱うアダプタを実装する。
4. no_std 構成向けに panic 以外のエラー経路（`Result` 返却等）を guardian に伝える手段を検討する。
