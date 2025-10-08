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
