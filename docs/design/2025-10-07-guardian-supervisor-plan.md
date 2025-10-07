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

### 現状と今後の拡張ポイント
- `SystemMessage::Watch(ActorId)` / `Unwatch(ActorId)` は 2025-10-07 実装済み。`ActorContext` が `watchers()` / `register_watcher()` を提供し、子生成時に親 `ActorId` を自動登録する。今後は、この情報を Terminated 通知や typed API へ橋渡しする。
- `Guardian::notify_failure` は `FailureInfo` を返し、`PriorityScheduler` が Escalate を蓄積する仕組みを導入済み。
- `PriorityScheduler::on_escalation` を追加し、外部ランタイムが FailureInfo をリアルタイムで受け取れるようになった。今後は、このハンドラを親 Guardian／system guardian に橋渡しする抽象レイヤ（例: `EscalationSink`）が必要。
- `map_system` を typed 層の DSL が差し替えられるよう、`TypedMailboxAdapter`（仮称）がクロージャ生成を担う。

## SystemMessage フロー（現状）
1. 子アクター生成時、`ActorContext::spawn_child` が `map_system` クロージャを `ChildSpawnSpec` にコピーし、次のディスパッチサイクルで `PriorityScheduler` が Guardian へ登録する。
2. 例外発生時（`std` feature 有効のみ）に `PriorityScheduler::dispatch_envelope` が `catch_unwind` で panic を捕捉し、`Guardian::notify_failure` を呼び出して `SystemMessage::Restart` / `Stop` を制御キューへ投入する。
3. 停止要求 (`SystemMessage::Stop`) は guardian から子に送信され、`PriorityEnvelope` の Control チャネル経由で最優先処理される。
4. no_std 構成では panic を捕捉できないため、今後 `Result` ベースのエラーパスや `SystemMessage::Failure` API を追加する余地がある。

## SystemMessage フロー（今後の計画）
1. Guardian が送出した `SystemMessage::Watch` / `Unwatch` を親アクターで経路制御する。現状 `ActorContext` の `watchers()` により監視者一覧を参照できるため、Terminated 通知処理と連動させる。
2. Escalate をサポートし、上位 Guardian へ転送する際も `map_system` クロージャを通じてメッセージ型を変換する。`PriorityScheduler::on_escalation` 経由で FailureInfo を受け取ったら、親 Guardian／system guardian へ `SystemMessage::Escalate` を送出する。
3. Typed 層が `map_system` を生成し、`SystemMessage` をユーザーの DSL に沿った挙動へマッピングできるようインターフェースを整備する。

## 高水準 API の現状
- `Props<M, R>`: MailboxOptions と `map_system` クロージャ、アクターハンドラを束ねる構造体。`Props::new` で簡単に生成できる。
- `ActorSystem<M, R>`: Scheduler を内包し、`root_context()` で `RootContext` を取得する。現在は `AlwaysRestart` のみサポート。
- `RootContext`:
  - `spawn(props)` でアクターを起動し、`PriorityActorRef` を返す。
  - `dispatch_next().await` や `run_until()` を利用して非同期にディスパッチを進める。同期環境向けには `blocking_dispatch_loop()` を提供。
- actor-std では `TokioFailureEventBridge` を通じて `FailureEventHub` を tokio broadcast に橋渡しできる。
- typed DSL の第一段階として `TypedProps` / `TypedActorSystem` / `TypedActorRef` を導入済み。
  - `TypedProps::new` でユーザーメッセージハンドラを登録し、内部的に `MessageEnvelope<User>` を生成する。
  - `TypedActorSystem::<U, _>::new(runtime)` で typed システムを構築し、`TypedRootContext::spawn` から typed アクターを起動できる。
  - `TypedActorRef::tell` でユーザーメッセージを型安全に送信し、SystemMessage は自動的に高優先度で処理される。
  - `PriorityActorRef` は priority-aware な低レベル API として現状の名前を維持し、`TypedActorRef` を高水準 API と位置付ける。（ドキュメント / プレリュードでレイヤの違いを明記する）

## 非同期ディスパッチ API ステータス（2025-10-07 更新）
- [x] `PriorityScheduler` に `dispatch_next` / `drain_ready_cycle` / `process_waiting_actor` を導入し、シグナル待機で次メッセージを `await` できるようにした。同期系 `dispatch_all` も内部で同じ処理経路を利用する。
- [x] `RootContext` / `TypedRootContext` / `TypedActorSystem` が `dispatch_next().await` を公開し、Tokio／embedded から非同期ディスパッチを直接呼び出せるようになった。
- [x] actor-std のユニットテストを `dispatch_next().await` ベースへ移行。Tokio MailboxRuntime との整合性確認済み。
- [x] actor-embedded テストで Condvar ベースの `block_on` を用意し、ローカルランタイムでもシグナル待機をスピン無しで駆動できるようにした。
- [x] Scheduler 向けの `run_forever()` / `blocking_dispatch_loop()` など高水準ランナを追加し、アプリケーションが簡単に常駐タスクを起動できるようにする。`PriorityScheduler` に常駐向け API 群を実装し、`scheduler_blocking_dispatch_loop_stops_with_closure` などのテストで挙動を確認済み。（2025-10-07）
- [x] actor-embedded については Embassy executor との統合ラッパ（例: `EmbassyActorSystem`）を整備し、`Spawner` から `dispatch_next` を起動するガイドを文書化する。`modules/actor-embedded/examples/embassy_run_forever.rs` と `docs/worknotes/2025-10-07-embassy-dispatcher.md` に手順をまとめた。（2025-10-07）
- [x] `dispatch_all` の段階的非推奨戦略を整理し、`docs/design/2025-10-07-dispatch-transition.md` に移行手順をまとめた。
## EscalationSink TODO リスト
- [x] `actor-core`: `SchedulerEscalationSink` をパブリック API として再編し、
      `trait EscalationSink`（handle 戻り値 `Result<(), FailureInfo>`）と具体実装
      （親ガーディアン／カスタム／合成）を提供する。`modules/actor-core/src/escalation.rs` に
      `ParentGuardianSink` / `CustomEscalationSink` / `CompositeEscalationSink` / `RootEscalationSink` を実装し、
      `lib.rs` から公開済み。（2025-10-07）
- [x] `PriorityScheduler`: Builder もしくは設定 API で EscalationSink を注入可能にし、
      子スケジューラがルートに参加する際に同一ポリシーを共有できるようにする。
      `PriorityScheduler::set_parent_guardian` / `on_escalation` / `set_root_escalation_handler` /
      `set_root_event_listener` を導入し、テストで動作確認済み。（2025-10-07）
- [ ] `Guardian`: `escalate_failure` の戻り値を拡張し、
      `SupervisorDirective::Stop`／`Restart` を返さなかったケースでも FailureInfo に
      サブタイプ（例: `EscalationStage`）を付与できるよう検討する。
- [x] `Props` / `ActorSystem` / `RootContext` 高水準 API を actor-core に追加し、Scheduler への直接依存なしに
      アクターを起動・メッセージ送信できるようにする。
- [ ] `ActorContext` / `ChildSpawnSpec`: `map_system` だけでなく EscalationSink のフックを
      親から子へ伝搬させ、Typed DSL で `SystemMessage::Escalate` を型安全に変換できるようにする。
- [ ] `TypedMailboxAdapter`（予定）: `SystemMessage::Failure` と `SystemMessage::Escalate` を
      ユーザー定義イベントへマップする仕組みを提供し、テストで Escalate→typed DSL の通知経路を検証する。
- [ ] `actor-core` テスト: 現状の `scheduler_escalation_chain_reaches_root` に加えて、
      カスタム EscalationSink が再試行を返した場合に scheduler が FailureInfo を再キューするケースを追加する。
- [x] `system_guardian` / `root_guardian`: ルート EscalationSink を定義し、最上位で FailureInfo を
      ログ／メトリクス／イベントストリームへ流すフックを整備する。
      `RootEscalationSink` が `tracing::error!`、`FailureEventHandler`、`FailureEventListener`
      を経由してログ・イベント配信できるようになり、`scheduler_root_escalation_handler_invoked` /
      `scheduler_root_event_listener_broadcasts` で検証。（2025-10-07）
- [x] `FailureEventHub` を std 構成で実装し、`PriorityScheduler::set_root_event_listener` から複数購読者へ配信できるようにした。

## Watch / Unwatch 親伝播設計草案

### 要件
- Guardian が子 mailbox に投入した `Watch` / `Unwatch` が親アクターにも届き、親側で監視対象リストを管理できること。
- Terminated 通知を受け取った親が `Unwatch` 済みの相手を除外しつつ、必要に応じて追加の SystemMessage（例: `Terminate`）を生成できること。

### 提案する変更
1. `ActorContext` に `handle_system_message`（仮称）を追加し、`PriorityScheduler` が制御メッセージをディスパッチする前に事前処理できるようにする。
2. `ActorContext` 内部に `WatchRegistry`（軽量な `BTreeSet<ActorId>` または `Shared<HashSet<_>>`）を保持し、`Watch` で insert、`Unwatch` で remove を実施する。
3. 親から子への `watch` API を公開する際は、`PriorityActorRef::try_send_system(SystemMessage::Watch)` を包むヘルパーを提供し、Typed 層でも同一ハンドラを利用できるようにする。
4. Terminated 受信時には `WatchRegistry` を参照して該当 watcher へ `SystemMessage::Terminated`（今後追加）を送出する経路を設計する。

- `EscalationSink` 抽象を導入し、`PriorityScheduler` が FailureInfo を渡すと親側で `SystemMessage::Failure` / `Escalate` を生成できるようにする。sink は `(FailureInfo) -> Result<(), QueueError<PriorityEnvelope<M>>>` のような戻り値を持たせ、失敗時に再キューイングが可能。

### Open Questions
- `Watch` / `Unwatch` は ProtoActor では `PID` を payload に持つ。Rust 実装では `map_system` を通じてユーザー型へ変換する必要があるため、`SystemMessage` に watcher 情報を追加するか、別途 `WatchEvent` 型を envelope で運ぶか検討する。
- 親通知用の SystemMessage を `ActorContext` レベルで消費するか、ユーザーにも透過するか（Akka Typed のように内部処理へ限定するか）を決める必要がある。
- no_std 環境でも `WatchRegistry` を活用できるよう、`Vec` + 線形検索で十分か、あるいは `heapless::IndexSet` 等を利用するかを評価する。

## Failure / Escalate 拡張方針

### 追加したメッセージ
- `SystemMessage::Failure(FailureInfo)`：Restart/Stop と同じチャネルで障害情報を保持（Escalate しない場合でもダンプ可能）。
- `SystemMessage::Escalate(FailureInfo)`：GuardianStrategy が Escalate を返した際、上位層へ伝播するためのメッセージ。現状は `PriorityScheduler::take_escalations` で収集する。
- `FailureInfo` には `actor: ActorId` と `reason: String` を保持。再起動統計などの拡張は今後追加予定。

### EscalationSink の構成案

```rust
pub enum EscalationTarget<M, R> {
  Scheduler,
  ParentGuardian {
    control_ref: PriorityActorRef<M, R>,
    map_system: Arc<dyn Fn(SystemMessage) -> M + Send + Sync>,
  },
  Custom(Box<dyn FnMut(&FailureInfo) -> Result<(), QueueError<PriorityEnvelope<M>>> + 'static>),
}

pub trait EscalationSink<M, R> {
  fn handle(&mut self, info: FailureInfo) -> Result<(), FailureInfo>;
}
```

- `PriorityScheduler` は `EscalationSink` を保持し、FailureInfo を Sink に渡す。戻り値が `Err(FailureInfo)` の場合はバッファに戻し、後続サイクルで再試行する。
- `EscalationTarget::ParentGuardian` は FailureInfo に含まれる `ActorPath` を `parent()` へ進め、親視点の FailureInfo を生成して `SystemMessage::Escalate` として親ガーディアンに送信する。
- `Guardian` 側には `escalate_failure(&FailureInfo) -> Result<Option<FailureInfo>, QueueError<_>>` を追加し、FailureInfo を再度 `notify_failure` へ流した際の指示（Restart/Stop/Escalate）を返す。

### ActorPath 仕様

- `ActorPath` は root から該当アクターまでの `ActorId` を順に保持する。
- `ActorPath::push_child` で子の ID を追加、`parent()` で親のパスを取得、`last()` で現在のアクター ID を求める。
- FailureInfo は常に自分自身の `ActorId` と `ActorPath` を保持し、親側では `escalate_to_parent()` でパスと ID を更新する。

### 予定する実装ステップ
1. `FailureInfo` に Restart 統計や最終処理メッセージなどのメタ情報を追加し、SupervisorStrategy が条件判定に利用できるようにする。
2. `GuardianStrategy::decide` で Escalate を返した際、`Guardian` が EscalationSink 経由で親アクターへ通知できるよう `PriorityScheduler::on_escalation` を活用する。（実装済み）
3. 親ガーディアンが `SystemMessage::Escalate` を受け取った際に `Guardian::notify_failure` を再実行できるよう、EscalationSink を `Scheduler` / `ParentGuardian` で切り替える抽象を追加する。
   - Sink では FailureInfo と再起動統計（今後導入）を保持し、親 strategy が Stop/Restart/Escalate を再評価できる。
   - ActorId を階層的に管理するため、`ActorPath`（例: Vec<ActorId>）を FailureInfo に含め、親 Guardian が自分の子を一意に特定できるようにする。
   - 子 Guardian 登録時に `ActorPath` を生成し、親への通知では `ActorPath::parent()` を使って FailureInfo を更新する。
4. `Supervisor` 実装に Failure/Escalate を通知するため、`ActorContext` に `notify_failure` フックを追加し、Typed 層でも `TypedSystemEvent::Failure` を処理できるようにする。
5. テストシナリオ: (a) 子アクター → 親 → ルートで Escalate が連鎖し、最終的に Stop 指示が届くケース。 (b) EscalationSink が親 Guardian の strategy によって Restart/Stop を再評価するケース。 (c) Escalate が system guardian 経由で最終処理へ到達する end-to-end テスト。

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

## 旧実装からのメモ（remote / cluster の参考ポイント）
- `modules/remote-core`: `RemoteRuntimeConfig` がトランスポート・シリアライザ・ブロックリストを束ねる。
      `RemoteTransport` は `connect` / `serve` を非同期で実装する契約になっているため、
      EscalationSink でリモート層へ渡す FailureInfo にはエンドポイント URI やトランスポート種別を
      含められるようメタデータ拡張を検討する。
- `modules/remote-std/src/endpoint_supervisor.rs`: `EndpointSupervisor` は writer / watcher のペアを生成し、
      `OneForOneStrategy::new(...).with_decider(|_| Directive::Stop)` で再起動ではなく停止を指示する。
      EscalationSink 導入後は endpoint の再生成・監視再登録ロジックを `SystemMessage::Escalate` に結び付け、
      WatchRegistry と FailureInfo を結合させる。
- `modules/cluster-std/src/cluster.rs`: `Cluster::ensure_remote` が `Remote` を起動し、
      `ClusterActivationHandler` 経由で仮想アクターの PID を解決する。クラスタ層では EscalationSink を
      利用して remote 失敗を `ClusterError::Provider` / `PartitionManagerError` などへ変換し、
      メンバーシップ更新や再接続ポリシーと連携させる。
      FailureEventHub を通じて cluster ハブが FailureEvent を購読し、監視ダッシュボードへ中継することを想定。
