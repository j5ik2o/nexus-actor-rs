# Embedded Runtime High-Level API Plan (2025-10-08)

## 目的

- `actor-std` と同等の「`tell` だけでディスパッチャが駆動される」ユーザー体験を `actor-embedded` でも提供する。
- ランタイム差し替え可能な共通抽象 (`FailureEventStream`, `MailboxRuntime`, `Timer`, `Spawn`) を活かし、std/embedded を同一コードパスで利用できる高レベル API を設計する。
- 既存のローレベルテストを内部契約検証に残しつつ、高レベル API 経由の統合テストを整備する。

## 現状整理

| 項目 | actor-std | actor-embedded |
| --- | --- | --- |
| スポーナー | `TokioSpawner` | `ImmediateSpawner` (`spawn` は同期実行) |
| タイマー | `TokioTimer` | `ImmediateTimer` (即時完了) |
| メールボックス | `TokioMailbox*` | `LocalMailbox` / `ArcMailbox` |
| 高レベル API | `ActorSystem` を Tokio ランタイム上で手動起動 | `ActorSystem` を手動で `dispatch_next` |

- どちらも `ActorSystem` の抽象は共有できているが、ユーザー側から見るとスケジューラの駆動を意識する必要がある。
- `actor-std` テストでは `TokioSpawner` を直接使用。`actor-embedded` テストでは `block_on` を自前実装して `dispatch_next` を手動呼び出し。

## 提案アーキテクチャ

```
ActorRuntimeDriver (trait)
  ├─ TokioRuntimeDriver      (actor-std)
  ├─ ImmediateRuntimeDriver  (actor-embedded, 単発タスク向け)
  └─ EmbassyRuntimeDriver    (actor-embedded, embassy-executor 連携)

ActorRuntimeDriver::spawn_actor_loop(...)  // actor_loop を内部で起動
ActorRuntimeDriver::pump()                 // 必要ならメインループに統合

ActorSystemFacade<R, Driver>
  -> 内部で ActorSystem<R, Driver::MailboxRuntime> を所有
  -> 外部 API: spawn, tell, stop, run_until_idle など
```

- `ActorRuntimeDriver` は `Spawn` と `Timer` を注入し、`ActorSystem` をラップする責務を負う。
- Embedded 向けには 2 実装を想定：
  - `ImmediateRuntimeDriver`: `ImmediateSpawner`/`ImmediateTimer` を使い、`pump()` 呼び出し毎にメッセージを 1 件処理する。
  - `EmbassyRuntimeDriver`: `embassy_executor` へタスク登録し、ハードウェアタイマや割り込みに乗せる。
- std/embedded 共通で `ActorSystemFacade`（仮称）を導入し、以下を提供：
  - `fn new(driver: Driver) -> Self`
  - `fn spawn<P>(&mut self, props: Props<...>) -> ActorRef`（背後で driver が必要なランタイムを起動）
  - `fn tell<A>(&self, actor: &ActorRef<A>, msg: A)`（既存の ActorRef API を継承）
  - `fn run_once(&mut self)` / `fn run_until_idle(&mut self)`（`ImmediateRuntimeDriver` 用）

### ActorRuntimeDriver trait 草案

```rust
pub trait ActorRuntimeDriver<M>
where
  M: Element,
{
  type Runtime: MailboxRuntime;
  type Spawn: Spawn;
  type Timer: Timer;
  type EventStream: FailureEventStream;

  /// ActorSystem 構築に必要な構成要素を取り出す。
  fn into_parts(self) -> (Self::Runtime, Self::Spawn, Self::Timer, Self::EventStream);

  /// 手動駆動が必要なランタイム向け。Tokio 等では `DriverState::Idle` を戻すだけでよい。
  fn pump(&mut self) -> DriverState;
}

pub enum DriverState {
  Busy,
  Idle,
}
```

- `M`（メッセージ型）に対してジェネリックなドライバを提供し、`ActorSystem<M, _>` を内部で生成できる設計。
- `Spawn`/`Timer`/`FailureEventStream` の関連型は、std/embedded で異なる実装を差し込む想定。
- `pump` 実装が不要な場合（Tokio など）は `DriverState::Idle` を返せば足りる。

### ActorSystemFacade スケルトン（案）

```rust
pub struct ActorRuntimeFacade<D, M>
where
  D: ActorRuntimeDriver<M>,
  M: Element,
{
  driver: D,
  system: ActorSystem<M, D::Runtime>,
}

impl<D, M> ActorRuntimeFacade<D, M>
where
  D: ActorRuntimeDriver<M>,
  M: Element,
{
  pub fn new(driver: D) -> Self {
    let (runtime, spawner, timer, stream) = driver.into_parts();
    let system = ActorSystem::with_components(runtime, spawner, timer, stream);
    Self { driver, system }
  }

  pub fn spawn<P>(&mut self, props: P) -> Result<ActorRef<M>, SpawnError>
  where
    P: Into<Props<M, D::Runtime>>,
  {
    self.system.root_context().spawn(props.into())
  }

  pub fn run_until_idle(&mut self) {
    while matches!(self.driver.pump(), DriverState::Busy) {}
  }
}
```

- `ActorSystem::with_components` は既存 `ActorSystem::new(runtime)` の拡張として追加が必要。
- `Props` まわりの型合わせは概念図。実装時に `Spawn` ・ `Timer` 連携の API 整備が必要。

## 実装ステップ

1. **抽象導入**
   - `modules/actor-core/src/api/runtime.rs` 付近に `ActorRuntimeDriver` trait（名前要検討）を追加。`Spawn`, `Timer`, `FailureEventStream` を束ね、`ActorSystem` をラップする最小 API を定義。
   - `actor-std`, `actor-embedded` でそれぞれ実装を提供。

2. **std/embedded ファサード作成**
   - `nexus-actor-std` に `TokioActorRuntime`（仮称）を追加し、既存テストを高レベル API 経由へ移行。低レベルテストは `tests/internal_*` として分離。
   - `nexus-actor-embedded` に `EmbeddedActorRuntime` を追加し、`LocalMailboxRuntime` ベースの簡易 executor (`pump` 呼び出しで 1 サイクル処理) を提供。

3. **統合テスト追加**
   - std/embedded 共通のテストケースを `tests/common_actor_runtime.rs` のような共有モジュールで用意し、双方のクレートからインポート。
   - 例: `tell` -> actor が状態更新 -> `run_until_idle` で確認。

4. **ドキュメント更新**
   - `docs/design` の API 棚卸しを更新し、高レベルファサード導入による利用フローを記述。
   - `README`/`CLAUDE.md` にサンプルコードを追加。

## メリットと懸念

- **メリット**
  - ユーザーはランタイム差を意識せず、Akka/Pekko 同等の UX を得られる。
  - 組み込みでも main-loop への `pump()` 連携など現場要件に合わせた拡張が容易。
  - `FailureEventStream` など既存の逆転依存を活かし、追加の循環依存を作らない。

- **懸念**
  - `ActorRuntimeDriver` 導入が抽象の層を増やすため過度な複雑化にならないよう API を最小化する。
  - 組み込み側のタイミング制御（割り込みとソフトウェアタスクの協調）をどう抽象化するか要検証。
  - `embassy_executor` 統合は追加機能フラグやサンプルが必要で、段階的導入が望ましい。

## 次タスク候補

1. `ActorRuntimeDriver` の詳細設計メモを `docs/design` に追記（メソッドシグネチャ案、所有関係図）。
2. `actor-std` に仮実装 `TokioActorRuntime` を追加し、既存テストを移行するスパイクを作成。
3. `actor-embedded` で `EmbeddedActorRuntime` の PoC を実装し、`run_until_idle` ベースのシンプルな統合テストを用意。
4. 共通テストハーネス／ドキュメント更新の具体的な TODO リストを CLAUDE.md に追記。

### EmbeddedActorRuntime PoC 事前整理

- **Mailbox 選定**: デフォルトは `LocalMailboxRuntime`、`embedded_arc` feature 有効時は `ArcMailboxRuntime` も差し替え可能にする。
- **Timer 実装**: `ImmediateTimer` を初期実装とし、RTIC / hardware timer 連携の hook を trait で拡張予定。
- **Spawner 戦略**: `ImmediateSpawner` をハンドラ登録用に利用し、`pump()` 呼び出しで Pending spawn を逐次実行。将来的に `embassy_executor` にブリッジする際は `spawn_embassy_dispatcher` を内部で呼び出すドライバを追加。
- **テスト構成**:
  - `#[cfg(feature = "embedded_arc")]` の組み合わせに対応した matrix テスト (local mailbox / arc mailbox)。
  - `run_until_idle` が `tell` 後に状態変化を観測できることを確認する統合テスト (`actor-embedded/tests/runtime_facade.rs` を新設予定)。
  - failure event の流れを確認する軽量テスト（`FailureEventStream` に `TestFailureEventStream` を利用）。
- **依存関係**: `nexus-actor-core-rs` の `ActorSystem` に `ImmediateTimer` と `ImmediateSpawner` を注入するコンストラクタ拡張（spike 実装時に別 PR で調整）。
- **リスク**: no_std 環境での `pump` 実装に伴う割り込み管理／クリティカルセクション扱い。`cortex-m` などターゲット別の abstraction 層を厚くしすぎないよう注意。

#### 差分棚卸し（2025-10-08 時点）

| 領域 | 現状 | PoC で必要な差分 | 備考 |
| --- | --- | --- | --- |
| `RuntimeComponents` | std 側のみで利用開始 | `ImmediateSpawner`/`ImmediateTimer`/`TestFailureEventStream` を束ねた `RuntimeComponents` を組み立てるヘルパを実装 | `no_std` でも利用できるよう `alloc` 前提で実装 |
| `EmbeddedActorRuntime`(仮) | 未定義 | `TokioActorRuntime` と同等の API (`spawn_actor`, `dispatch_next`, `pump`, `run_until_idle`) を提供する struct を `actor-embedded` に追加 | `RuntimeComponents::new(LocalMailboxRuntime::default(), ...)` を利用 |
| Failure Event Stream | テスト用途のみ | `actor-embedded` で `TestFailureEventStream` を再利用 or 軽量版 `EmbeddedFailureEventHub` を実装 | `alloc` 版 Mutex を使う必要あり (`spin::Mutex` または `nexus_utils_embedded_rs` 既存構造) |
| テスト | 旧来の手動 `block_on` のみ | 新 API 経由の統合テスト (`tests/runtime_driver.rs`) を追加し、`tell` -> `run_until_idle` を検証 | `std` feature 時のみコンパイル (既存テストと同様に `extern crate std`) |
| ドキュメント | 高レベル案のみ | README/CLAUDE へ新 API の利用方法を追記 | std/embedded 共通のコード例を提供 |
| 将来拡張 | embassy 連携メモのみ | `spawn_embassy_dispatcher` を `RuntimeComponents` と接続する設計メモを追加 | 実装は PoC 後で可 |

#### EmbeddedActorRuntime 実装タスク候補

1. `actor-embedded` に `runtime_driver.rs` を追加し、`EmbeddedActorRuntime` と `DriverState` を実装する。
2. `RuntimeComponents` を用いた `EmbeddedActorRuntime::new()` を作成し、`FailureEventStream` として `TestFailureEventStream` の `no_std` 版（`EmbeddedFailureEventHub`）を実装。
3. `spawn_actor` / `dispatch_next` / `run_until_idle` / `pump` を提供し、`ImmediateSpawner` では未処理タスクをキューに蓄え、`pump` で順次実行する仕組みを導入。
4. `modules/actor-embedded/tests/runtime_driver.rs` を作成し、`tell` → `run_until_idle` の正常系、`pump` の単体挙動、`FailureEventStream` の配信確認などを盛り込む。
5. `docs/design/2025-10-08-embedded-runtime-plan.md` に embassy 連携と `run_until_idle` API 設計の詳細を追記し、PoC 後の発展タスクを整理する。

### ActorRuntimeFacade Trait 案

共通の高レベル API を提供するため、以下のようなファサード trait を導入する計画。

```rust
pub trait ActorRuntimeFacade<U>
where
  U: Element,
{
  type Runtime;
  type EventStream: FailureEventStream;

  fn event_stream(&self) -> &Self::EventStream;
  fn spawn_actor(&mut self, props: Props<U, Self::Runtime>)
    -> Result<ActorRef<U, Self::Runtime>, QueueError<PriorityEnvelope<MessageEnvelope<U>>>>;
  fn dispatch_next(
    &mut self,
  ) -> impl Future<Output = Result<(), QueueError<PriorityEnvelope<MessageEnvelope<U>>>>> + Send;
  fn pump(&mut self) -> DriverState;

  fn run_until_idle(&mut self) -> impl Future<Output = Result<(), QueueError<PriorityEnvelope<MessageEnvelope<U>>>>> + Send
  where
    Self: Sized,
  {
    async move {
      while matches!(self.pump(), DriverState::Busy) {
        self.dispatch_next().await?;
      }
      Ok(())
    }
  }
}
```

- `TokioActorRuntime` では `dispatch_next` が単に `ActorSystem::dispatch_next()` を呼び、`pump` は常に `DriverState::Idle`。
- `EmbeddedActorRuntime` は `pump` で未処理タスクの存在を返し、`run_until_idle` でメインループ相当の処理を表現できる。
- この抽象により、アプリケーションは `ActorRuntimeFacade` をジェネリック引数として受け取り、std/embedded の切り替えを透過化できる。例: `fn drive<F: ActorRuntimeFacade<U>>(mut facade: F) { facade.run_until_idle().await; }`

今後のタスク:

1. `ActorRuntimeFacade` trait を専用モジュール（`actor-core` もしくは `actor-runtime` 新クレート）に追加し、共有ユーティリティを提供する。
2. `TokioActorRuntime` と `EmbeddedActorRuntime` に対し trait 実装を追加し、テストを trait ベースにリファクタリングする。
3. `pump` の意味付けと `ImmediateSpawner` の動作を整理し、埋め込み向けに「忙しい状態」を判定できるよう実装を拡張する。

### ActorSystem 常駐タスクと CoordinatedShutdown 検討メモ

- **Tokio 環境**
  - `ActorSystem::start()`（仮）を追加し、内部で `tokio::spawn(async move { system.run_forever().await })` を実行する。
  - 返り値として `JoinHandle<()>` と停止トークン（例: `ActorSystemShutdown`）を保持し、`CoordinatedShutdown` 初期実装では `handle.abort()` で強制停止、将来的には Graceful に移行。
  - `Dispatcher` への登録は `Spawner` を通すのではなく、`ActorSystem` が自ら常駐する形にするため、既存 `TokioSpawner` はユーザーアクター用タスク向けに限定し、スケジューラ本体は `start()` で確実に Tokio スケジューラへ載せる。

- **Embedded 環境**
  - `EmbeddedActorRuntime::run_until_idle()` をアプリケーションのメインループ（割り込み駆動 or cooperative loop）から呼び、`pump()` の戻り値で終了条件を判断できるようにする。
  - 将来的に `CoordinatedShutdown` と連携するため、`run_until_idle()` が停止フラグ（例: `AtomicBool`) を参照して早期リターンできる構造を検討。

- **CoordinatedShutdown の初期方針**
  - 第一段階では、std 環境で SIGTERM/SIGINT を受け取った際に `ActorSystemShutdown::trigger()`（仮）を呼び、Tokio タスクを `abort()` して終了ログを出力する簡易実装を目指す。
  - Embedded 環境では `pump()` が `DriverState::Idle` を返したタイミングで停止可能。将来的な拡張として、外部から停止要求を受けて `run_until_idle()` が完了した時点でメインループを抜ける設計にする。
  - 本格的な「段階的シャットダウン」（クラスタ協調や永続ストア flush 等）は次フェーズで検討し、現段階では「安全に停止するフックを提供する」ことをゴールとする。
