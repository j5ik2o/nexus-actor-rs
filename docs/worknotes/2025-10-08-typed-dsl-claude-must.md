# Claude Code 向け Typed DSL MUST 作業指示

## 背景と目的
- [`docs/design/2025-10-07-typed-actor-plan.md`](docs/design/2025-10-07-typed-actor-plan.md:1) の TODO #2 / #3 が未完（TypedActorAdapter + map_system 経路）。
- 現実装は [`MessageEnvelope`](modules/actor-core/src/typed.rs:16) を介してユーザーメッセージを処理できるが、`SystemMessage` が Typed DSL へ十分に写像されていない。
- Guardian / Scheduler が `map_system` を経由して制御メッセージを配送しているため、Typed 層が適切にクロージャを供給しなければ統合テストを構築できない。

## 現状概要（要参照）
- [`TypedContext`](modules/actor-core/src/typed.rs:27) と [`Behavior`](modules/actor-core/src/typed.rs:76) は stateless DSL を処理できる状態。
- [`TypedProps::with_behavior_and_system`](modules/actor-core/src/typed.rs:157) は `map_system = Arc::new(|sys| MessageEnvelope::System(sys))` で固定化されており、ユーザー定義 enum への写像ができない。
- Guardian / Scheduler は `map_system` を以下の経路で依存:
  - [`PriorityScheduler::spawn_actor`](modules/actor-core/src/system.rs:86)
  - [`ChildSpawnSpec`](modules/actor-core/src/context.rs:80)
  - [`Guardian::register_child`](modules/actor-core/src/guardian.rs:87)
  - [`PriorityScheduler::forward_to_parent_guardian`](modules/actor-core/src/scheduler.rs:207)
- 統合テストは [`typed_actor_system_handles_user_messages`](modules/actor-core/src/typed.rs:366) のみで、SystemMessage 経路の検証が欠如。

## 作業タスク

### Task 1 — TypedActorAdapter 実装と統合
1. 新規構造体 `TypedActorAdapter` を [`modules/actor-core/src/typed.rs`](modules/actor-core/src/typed.rs) に追加。狙いは `Behavior` と `TypedContext` の橋渡し、および `map_system` クロージャ生成。
2. `TypedActorAdapter` の責務:
   - `Behavior` と任意の内部状態（stateful 対応）を保持。
   - `handle_user(&mut ActorContext, MessageEnvelope::User)` を提供し、内部で [`TypedContext`](modules/actor-core/src/typed.rs:27) を生成して `Behavior` を駆動。
   - `map_system(&Arc<dyn Fn(SystemMessage) -> MessageEnvelope<U>>)` を返却。`Behavior` に `SystemMessage` ハンドラが存在する場合は適切な enum へマップ。
3. `TypedProps::with_behavior_and_system` を `TypedActorAdapter` で書き換え:
   - `Behavior` / `system_handler` を Adapter に委譲。
   - `Props::new` へ渡す `handler` クロージャは Adapter の `handle_user` / `handle_system` を呼ぶだけにする。
4. `TypedProps::from_typed_handler` / `with_behavior` / `with_system_handler` からは Adapter を通じて統一的に処理。

### Task 2 — map_system 生成の型安全化
1. 既存 `MessageEnvelope::System(SystemMessage)` を置換し、ユーザー定義 enum / `TypedSystemEvent` へ射影できる仕組みを導入。
2. `map_system` の戻り値は `Arc<dyn Fn(SystemMessage) -> MessageEnvelope<U> + Send + Sync>` を維持しつつ、Typed 層が提供するクロージャを差し込む:
   - Stateless Actor 例: `Arc::new(|sys| Message::System(TypedSystemEvent::from(sys)))` のようにラップ。
   - Stateful Actor 例: state へアクセスは不可のため、`SystemMessage` → enum 変換のみをクロージャで行い、stateful 処理は `handle_system` 内で行う。
3. Guardian / Scheduler 側のハンドリングは既存通りで問題ないが、`SystemMessage::Escalate` が正しい優先度で配信されることを Adapter 側テストで保証する。

### Task 3 — 統合テスト拡張
1. [`modules/actor-core/src/typed.rs`](modules/actor-core/src/typed.rs) の `#[cfg(test)]` セクションを拡張:
   - `test_typed_actor_handles_system_stop`（仮称）: `SystemMessage::Stop` を `map_system` が typed enum に変換し、`Behavior` 側で停止フラグをセットできること。
   - `test_typed_actor_notifies_watchers`（仮称）: 書き換えた `map_system` が `SystemMessage::Watch` を typed イベントへ変換し、watchers が `ActorContext::register_watcher` を通じて反映されること。
   - `test_typed_actor_stateful_behavior`（仮称）: `Behavior` が stateful で `SystemMessage::Failure` などを受け、内部状態を更新するケース。
2. テスト内では [`PriorityActorRef::try_send_control_with_priority`](modules/actor-core/src/context.rs:52) を利用し、制御メッセージが適切な優先度で処理されることを確認。
3. 可能であれば `TestMailboxFactory` を使った end-to-end シナリオで `Escalate` → 親 Guardian 経路を検証。

## 実装メモ
- `TypedActorAdapter` が `Arc` や `Box` を保持する際、`Send + Sync` 制約を維持すること。mailbox runtime は `Clone` を要求する点に注意。
- `SystemMessage` の優先度は [`SystemMessage::priority`](modules/actor-core/src/mailbox.rs)（※新規参照が必要）で決定済み。Typed 層で上書きしないこと。
- `Behavior` の `handler` は `for<'r,'ctx>` で定義済みのため、Adapter が `TypedContext` を生成する際は lifetime を崩さないよう注意。

## テストと品質保証
- 実装後に以下コマンドを **必ず** 実行して成功を確認:
  - `cargo +nightly fmt`
  - `cargo clippy --workspace --all-targets --all-features`
  - `cargo test -p nexus-actor-core-rs`
  - `cargo test -p nexus-actor-core-rs --features std`
- 追加テストでは panic / unwrap を避け、`Result` は `expect` で理由を明示。

## ドキュメント更新
- 作業完了後に [`docs/design/2025-10-07-typed-actor-plan.md`](docs/design/2025-10-07-typed-actor-plan.md:79) の TODO #2 / #3 を完了扱いへ更新し、関連知見があれば新たな節を追加。
- `progress.md` に進捗ログを追記。

## 完了条件（Definition of Done）
1. `TypedActorAdapter` と `map_system` 改修がマージされ、Typed DSL が SystemMessage を型安全に扱える。
2. 上記テスト群が追加され、`cargo test` 系がすべて成功。
3. ドキュメント・進捗ログが更新され、次タスクへ引き継げる状態を整備。
4. Lint（clippy）とフォーマット（rustfmt）で警告が出ない。

以上のガイドに基づき、Claude Code で実装を進めてください。
