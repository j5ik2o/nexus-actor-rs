# Embedded Runtime High-Level API Plan (2025-10-08)

## 目的

- `actor-std` と同等の「`tell` だけでディスパッチャが駆動される」ユーザー体験を `actor-embedded` でも提供する。
- ランタイム差し替え可能な共通抽象 (`FailureEventStream`, `MailboxFactory`, `Timer`, `Spawn`) を活かし、std/embedded を同一コードパスで利用できる高レベル API を設計する。
- 既存のローレベルテストを内部契約検証に残しつつ、高レベル API 経由の統合テストを整備する。

## 現状整理

| 項目 | actor-std | actor-embedded |
| --- | --- | --- |
| スポーナー | `TokioSpawner` | `ImmediateSpawner` (`spawn` は同期実行) |
| タイマー | `TokioTimer` | `ImmediateTimer` (即時完了) |
| メールボックス | `TokioMailbox*` | `LocalMailbox` / `ArcMailbox` |
| 高レベル API | `ActorSystemRunner` + `TokioSystemHandle` | `ActorSystemRunner` / `run_until_idle()` + `Behaviors` DSL |
| Deprecated ラッパ | なし（`TokioActorRuntime` は撤去済み） | なし（`EmbeddedActorRuntime` は撤去済み） |

- すべての利用コードを `ActorSystem` + `ShutdownToken` + SystemHandle で統一済み。旧ラッパは 2025-10-08 のクリーンアップで削除済み。
- std 環境では `TokioSystemHandle::start_local(runner)` を通じて scheduler 常駐タスクを起動し、`spawn_ctrl_c_listener()` で SIGINT/SIGTERM（現状は ctrl-c）を監視して `ShutdownToken::trigger()` を呼ぶ。
- embedded 環境では `run_until_idle()` をアプリ側のメインループから呼び出し、停止トークンを監視して安全に停止する。

## 提案アーキテクチャ

1. **ActorSystemRunner + SystemHandle** を中核に据える。
   - `ActorSystem::from_parts` で `ActorSystem` を構築し、`Behaviors` DSL（`supervise`, `message_adapter` など）でアクターを定義してから `into_runner()` で常駐用ランナーを取得する。
   - std 向けには `TokioSystemHandle::start_local(runner)` を提供し、`tokio::task::spawn_local` により scheduler を常駐させる。`ShutdownToken` は `ctrl_c` 監視経由でトリガーする。
   - embedded 向けには runner を直接扱い、アプリ側メインループが `run_until_idle()` + `shutdown_token()` を組み合わせて制御する。

2. **ランタイム ラッパの撤去後整理**。
  - `TokioActorRuntime` / `EmbeddedActorRuntime` は削除済み。新規 API 利用例は Runner + SystemHandle を直接扱う形で整備する。
  - 今後は外部コードも含め Runner ベースへ移行させ、不要なラッパを再導入しない方針を維持する。

3. **CoordinatedShutdown の整理**。
  - Tok io: `TokioSystemHandle::start_local(runner)` を公開済み。今後 `start(runner)`（`tokio::spawn` ベースで `Send` ランナーを扱う）を追加し、`tokio::signal::unix::signal` を使って SIGTERM/SIGINT に段階的に対応する。`spawn_ctrl_c_listener()` は最小構成として残しつつ、Graceful Stop → Timeout → Abort のフローを検討する。
  - Embedded: アプリケーションのメインループで `run_until_idle()` を呼び出し、外部からの停止トリガを `ShutdownToken` で受け取るサンプルを README/CLAUDE.md に掲載する。必要に応じて `EmbeddedSystemHandle`（仮）のような helper を用意し、停止フラグと `run_until_idle` を統合する。

## 実装ステップ

1. **抽象導入**
   - `modules/actor-core/src/api/runtime.rs` 付近に `ActorRuntimeDriver` trait（名前要検討）を追加。`Spawn`, `Timer`, `FailureEventStream` を束ね、`ActorSystem` をラップする最小 API を定義。
   - `actor-std`, `actor-embedded` でそれぞれ実装を提供。

2. **std/embedded ファサード整備**
  - `nexus-actor-std` では `TokioSystemHandle` を中心としたドキュメントとサンプルを更新し、Runner ベース API への移行を完了させる。
  - `nexus-actor-embedded` はアプリケーションが `ActorSystemRunner::run_until_idle()` を直接呼び出すガイドを整備し、必要に応じて小規模ヘルパー（例: `EmbeddedSystemDriver`）を検討する。

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
2. Runner + SystemHandle ベースのテスト/サンプルを `actor-std` / `actor-embedded` 双方で整備し、旧ラッパに依存したコードの残存を洗い出す。
3. 共通テストハーネス／ドキュメント更新の具体的な TODO リストを CLAUDE.md に追記。

### Embedded Runner 運用メモ

- **Mailbox / Timer / Spawner**: デフォルト構成は `LocalMailboxFactory` + `ImmediateSpawner` + `ImmediateTimer`。`embedded_arc` feature 有効時は `ArcMailboxFactory` も選択可能。
- **FailureEventHub**: 軽量な `EmbeddedFailureEventHub` を維持し、`FailureEventStream` を利用するコードと Runner を直接接続する。
- **運用モデル**: アプリケーションは `ActorSystemRunner::run_until_idle()` を周期的に呼び出し、`ShutdownToken` を監視して安全に停止する。旧来の `pump()` ベース API は廃止済み。
- **テスト**: `actor-embedded/tests/runtime_driver.rs` で Runner ベースの統合テストを継続し、`tell` → `run_until_idle` → 状態検証の流れを確保する。
- **将来拡張**: `embassy_executor` や hardware timer との統合が必要になった場合は、Runner/`ShutdownToken` ベースを前提にした追加ドライバ（例: `EmbeddedSystemDriver`）を設計する。

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
    -> Result<ActorRef<U, Self::Runtime>, QueueError<PriorityEnvelope<RuntimeMessage>>>;
  fn dispatch_next(
    &mut self,
  ) -> impl Future<Output = Result<(), QueueError<PriorityEnvelope<RuntimeMessage>>>> + Send;
  fn pump(&mut self) -> DriverState;

  fn run_until_idle(&mut self) -> impl Future<Output = Result<(), QueueError<PriorityEnvelope<RuntimeMessage>>>> + Send
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

- Runner/Handle ベースの現在のアーキテクチャでは、`ActorRuntimeFacade` を導入する場合でも内部で `ActorSystemRunner` と環境固有のハンドルをラップする形になる想定。trait 導入の是非は引き続き要検討。

今後のタスク:

1. `ActorRuntimeFacade` trait を導入するかどうかを再評価し、導入する場合は Runner/Handle ベースの最小 API を設計する。
2. Facade を実装する際は `TokioSystemHandle` や embedded 用ドライバとの対応関係を整理し、共通テストハーネスで評価する。
3. `ImmediateSpawner`／`ImmediateTimer` の挙動ドキュメントを更新し、Facade なしでも Runner を扱いやすくするためのヘルパー整備を検討する。

### ActorSystem 常駐タスクと CoordinatedShutdown 検討メモ

- **Tokio 環境**
  - `ActorSystem::start()`（仮）を追加し、内部で `tokio::spawn(async move { system.run_forever().await })` を実行する。
  - 返り値として `JoinHandle<()>` と停止トークン（例: `ActorSystemShutdown`）を保持し、`CoordinatedShutdown` 初期実装では `handle.abort()` で強制停止、将来的には Graceful に移行。
  - `Dispatcher` への登録は `Spawner` を通すのではなく、`ActorSystem` が自ら常駐する形にするため、既存 `TokioSpawner` はユーザーアクター用タスク向けに限定し、スケジューラ本体は `start()` で確実に Tokio スケジューラへ載せる。

- **Embedded 環境**
  - アプリケーションのメインループ（割り込み駆動 / cooperative loop）で `ActorSystemRunner::run_until_idle()` を呼び、`ShutdownToken` を監視して終了させる運用を前提とする。
  - 将来的に `CoordinatedShutdown` と連携するため、`run_until_idle()` が停止フラグ（例: `AtomicBool`）を参照して早期リターンできる構造や簡易ドライバの追加を検討する。

- **CoordinatedShutdown の初期方針**
  - 第一段階では、std 環境で SIGTERM/SIGINT を受け取った際に `ActorSystemShutdown::trigger()`（仮）を呼び、Tokio タスクを `abort()` して終了ログを出力する簡易実装を目指す。
  - Embedded 環境では `run_until_idle()` が停止検知後に早期復帰できるようにし、`ShutdownToken` 経由で停止要求を処理する。`pump()` への依存は廃止済み。
  - 本格的な「段階的シャットダウン」（クラスタ協調や永続ストア flush 等）は次フェーズで検討し、現段階では「安全に停止するフックを提供する」ことをゴールとする。

### SystemDriver 抽象の導入案

- `actor-core` 側に以下のような trait を追加し、`ActorSystem` から委譲する。

```rust
pub trait SystemDriver<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
{
  type Handle;

  fn start(self) -> Self::Handle;
  fn shutdown(handle: &Self::Handle, token: &ShutdownToken);
}
```

- `ActorSystem::into_runner()` で driver に渡す前提の構造体を取得し、std/embedded で個別に `SystemDriver` を実装する。
- `ShutdownToken` は `actor-core` が提供し、driver 側が `trigger()` や `is_triggered()` を監視する。
- `TokioSystemDriver`（`actor-std`）は `tokio::spawn` に `ActorSystemRunner::run_forever()` を渡し `JoinHandle` を保持、SIGTERM/SIGINT を `tokio::signal::ctrl_c()` で拾って `ShutdownToken::trigger()` を呼ぶ。
- Embedded 側は driver を持たず、アプリのメインループが `run_until_idle()`／`pump()` を直接呼ぶ形でも問題ないため、driver trait 実装はオプション扱いとする。
- これにより `ActorSystem` 自体は tokio を知らず、std 側／embedded 側で適切な driver を提供する設計に切り替える。
