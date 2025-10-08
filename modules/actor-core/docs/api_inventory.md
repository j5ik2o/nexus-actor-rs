# actor-core API/SPI 棚卸し（MECE分類・2025-10-08）

本ドキュメントは actor-core の公開 API と内部 SPI を MECE（Mutually Exclusive, Collectively Exhaustive）に分類し、再編タスクの基準とする。

## API レイヤ（ユーザー向け公開インターフェイス）

| カテゴリ | シンボル | 実配置 | 備考 |
| --- | --- | --- | --- |
| actor | `ActorRef`, `ActorSystem`, `Props`, `Behavior`, `TypedContext`(旧`Context`), `RootContext` | `modules/actor-core/src/api/actor/*.rs` | API 基本操作レイヤ（TypedContext は後日リネーム予定） |
| messaging | `MessageEnvelope` | `modules/actor-core/src/api/messaging/message_envelope.rs` | ユーザーメッセージとシステムメッセージの橋渡し |
| identity | `ActorId`, `ActorPath` | `modules/actor-core/src/api/identity/{actor_id.rs,actor_path.rs}` | ルーティング／名前解決用 ID 型 |
| runtime | `Mailbox`, `MailboxRuntime`, `MailboxSignal`, `PriorityEnvelope`, `SystemMessage`, `Spawn`, `Timer` | `modules/actor-core/src/api/runtime.rs`（実体は `runtime/mailbox/*`, `spawn.rs`, `timer.rs`） | std/embedded 両対応の抽象境界 |
| supervision | `Supervisor`, `SupervisorDirective`, `NoopSupervisor`, `FailureEvent`, `EscalationStage`, `EscalationSink`, `FailureEventHandler`, `FailureEventListener`, `RootEscalationSink` | `modules/actor-core/src/api/supervision/*.rs` | ユーザー拡張ポイントとして公開する監督/失敗ハンドラ |
| shared | `Shared`, `StateCell` | 外部クレート (`nexus_utils_core_rs`) を `api/shared.rs` で再エクスポート | 共有状態抽象 |
| event_stream | `FailureEventStream` | `modules/actor-core/src/api/event_stream.rs` | 実装は `actor-std` / `actor-embedded` など外部クレート側で提供 |

## Runtime レイヤ（内部実装・pub(crate)）

| カテゴリ | シンボル | 実配置 | 備考 |
| --- | --- | --- | --- |
| context | `ActorContext`, `ChildSpawnSpec`, `InternalActorRef` | `modules/actor-core/src/runtime/context/{actor_context.rs,child_spawn_spec.rs,internal_actor_ref.rs}` | API 側では `crate::runtime::context` 経由で参照 |
| system | `InternalActorSystem`, `InternalRootContext`, `InternalProps` | `modules/actor-core/src/runtime/system/{internal_actor_system.rs,internal_root_context.rs,internal_props.rs}` | スケジューラ／ガーディアン連携の中核 |
| mailbox | `PriorityEnvelope`, `QueueMailbox*`, `MailboxOptions`, `SystemMessage` | `modules/actor-core/src/runtime/mailbox/{messages.rs,queue_mailbox.rs,traits.rs}` | API からは `api::runtime` を介して公開可否を制御 |
| scheduler | `PriorityScheduler`, `ActorCell` | `modules/actor-core/src/runtime/scheduler/{priority_scheduler.rs,actor_cell.rs}` | 優先度スケジューラ本体（外部には未公開） |
| guardian | `Guardian`, `GuardianStrategy` 実装, `ChildRecord` | `modules/actor-core/src/runtime/guardian/{guardian.rs,strategy.rs,child_record.rs}` | API には戦略インターフェイスのみ再公開予定 |
| supervision | `CompositeEscalationSink`, `CustomEscalationSink`, `ParentGuardianSink`, `RootEscalationSink` 等 | `modules/actor-core/src/runtime/supervision/{parent_guardian_sink.rs,root_sink.rs,composite_sink.rs,custom_sink.rs,traits.rs}` | Root/Parent ガーディアン向け内部シンク（API には trait/handler のみ公開） |

## Platform 層（Feature 切替境界）

| feature | シンボル | 実配置 | 備考 |
| --- | --- | --- | --- |
| `std` | `ActorSystem::blocking_dispatch_*` | `modules/actor-core/src/runtime/system/internal_actor_system.rs` | std ランタイム限定 API。イベントストリーム実装は外部クレートに移設 |
| `alloc` | なし（共通化済み） | - | 現時点では共通コードで提供 |

## 公開可否ポリシー指針

- API レイヤは `pub`、Runtime/Platform は原則 `pub(crate)` で閉じる。
- Runtime の型を API が利用する場合は型 alias または専用 wrapper で公開する（例: `api::runtime::MailboxRuntime` は trait のみ公開し、具象型は内部）。
- `SystemMessage` は外部に晒さず、ユーザーは `MessageEnvelope::system()` のようなヘルパー経由で扱う設計に改める。

この分類を基準に、以降のステップでファイル配置と可視性を段階的に移行する。
