# actor-core API棚卸し (2025-10-08)

| シンボル | 現行定義 (ファイル) | 現状公開レベル | 提案区分 | 備考 |
| --- | --- | --- | --- | --- |
| ActorAdapter | src/actor/behavior.rs | `pub` | API/actor | 振る舞い差し替え用アダプタ |
| ActorRef | src/actor/actor_ref.rs | `pub` | API/actor | エンドユーザーの主要参照型 |
| ActorSystem | src/actor/system.rs | `pub` | API/system | 構築器はAPI、内部実装はruntimeへ分離予定 |
| Behavior | src/actor/behavior.rs | `pub` | API/actor | メッセージハンドラ定義 |
| Context | src/actor/context.rs | `pub` | API/actor | アクター実行時コンテキスト |
| MessageEnvelope | src/actor/message_envelope.rs | `pub` | API/messaging | メッセージ転送のラッパ |
| Props | src/actor/props.rs | `pub` | API/actor | アクター生成設定 |
| RootContext | src/actor/root_context.rs | `pub` | API/system | エントリポイント、runtime::RootContextと層分離予定 |
| ActorId | src/actor_id.rs | `pub` | API/identity | ID生成器、Shared抽象で置換予定 |
| ActorPath | src/actor_path.rs | `pub` | API/identity | 名前解決兼ID表現 |
| CompositeEscalationSink | src/escalation/composite_sink.rs | `pub` | API/supervision | 複数Sink連結 |
| CustomEscalationSink | src/escalation/custom_sink.rs | `pub` | API/supervision | ユーザー実装ベース |
| EscalationSink | src/escalation/traits.rs | `pub` | API/supervision | 失敗通知IF |
| FailureEventHandler | src/escalation/traits.rs | `pub` | API/supervision | Sink向けハンドラ |
| FailureEventListener | src/escalation/traits.rs | `pub` | API/supervision | イベント購読者 |
| ParentGuardianSink | src/escalation/parent_guardian_sink.rs | `pub` | Runtime/supervision | ガーディアン内部向け、最終的にinternal予定 |
| RootEscalationSink | src/escalation/root_sink.rs | `pub` | Runtime/supervision | Root guardian専用、internal化予定 |
| EscalationStage | src/failure/escalation_stage.rs | `pub` | API/failure | 失敗段階状態 |
| FailureEvent | src/failure/failure_event.rs | `pub` | API/failure | 失敗イベント本体 |
| FailureInfo | src/failure/failure_info.rs | `pub` | API/failure | 失敗詳細メタデータ |
| FailureMetadata | src/failure/metadata.rs | `pub` | API/failure | 失敗メタ情報拡張 |
| FailureEventHub | src/failure_event_stream.rs | `pub` (std) | Platform/std | std限定イベント配信、platform::stdへ移動 |
| FailureEventSubscription | src/failure_event_stream.rs | `pub` (std) | Platform/std | 同上 |
| AlwaysRestart | src/guardian/strategy.rs | `pub` | Runtime/supervision | 内部デフォルト戦略、external公開から削除検討 |
| Guardian | src/guardian/guardian.rs | `pub` | Runtime/supervision | 実行系内部向け、internal化予定 |
| GuardianStrategy | src/guardian/strategy.rs | `pub` | Runtime/supervision | スーパーバイザ戦略の一部、公開可否要検討 |
| SystemMessage | src/mailbox/messages.rs | `pub` | Runtime/messaging | システム専用メッセージ、internal化予定 |
| Mailbox | src/mailbox/traits.rs | `pub` | API/runtime | ユーザー定義メールボックス向けIF |
| MailboxOptions | src/mailbox/queue_mailbox.rs | `pub` | API/runtime | メールボックス構成 |
| MailboxPair | src/mailbox/queue_mailbox.rs | `pub` | Runtime/messaging | ランタイム内部、APIからは隠蔽予定 |
| MailboxRuntime | src/mailbox/traits.rs | `pub` | API/runtime | ランタイム抽象、Shared抽象と統合検討 |
| MailboxSignal | src/mailbox/traits.rs | `pub` | API/runtime | 埋め込み向け通知IF |
| PriorityEnvelope | src/mailbox/queue_mailbox.rs | `pub` | Runtime/messaging | スケジューラ内部優先度用 |
| QueueMailbox | src/mailbox/queue_mailbox.rs | `pub` | Runtime/messaging | デフォルト具象、APIからはplatform::queueとして公開予定 |
| QueueMailboxProducer | src/mailbox/queue_mailbox.rs | `pub` | Runtime/messaging | internal化予定 |
| QueueMailboxRecv | src/mailbox/queue_mailbox.rs | `pub` | Runtime/messaging | internal化予定 |
| Shared | `nexus_utils_core_rs` | `pub` | API/shared | 共有状態抽象 |
| StateCell | `nexus_utils_core_rs` | `pub` | API/shared | 共有セル抽象 |
| PriorityScheduler | src/scheduler/priority_scheduler.rs | `pub` | Runtime/scheduler | 実装詳細、internal化予定 |
| Spawn | src/spawn.rs | `pub` | API/system | API経由で再提供 |
| NoopSupervisor | src/supervisor.rs | `pub` | API/supervision | カスタムSupervisor用デフォルト |
| Supervisor | src/supervisor.rs | `pub` | API/supervision | スーパーバイザIF |
| SupervisorDirective | src/supervisor.rs | `pub` | API/supervision | 指示列挙 |
| Timer | src/timer.rs | `pub` | API/runtime | std/alloc両対応の境界に配置 |
| actor_loop | src/lib.rs | `pub` | API/system | 共通ランループ、platform経由で再公開予定 |
| ActorContext | src/context/actor_context.rs | `pub` | Runtime/context | 内部実装に移行予定 |
| ChildSpawnSpec | src/context/child_spawn_spec.rs | `pub` | Runtime/context | APIから非公開化予定 |
| InternalActorRef | src/context/internal_actor_ref.rs | `pub(crate)` | Runtime/context | 内部維持 |
| InternalActorSystem | src/system/internal_actor_system.rs | `pub(crate)` | Runtime/system | 内部維持 |
| InternalProps | src/system/internal_props.rs | `pub(crate)` | Runtime/system | 内部維持 |
| InternalRootContext | src/system/internal_root_context.rs | `pub(crate)` | Runtime/system | 内部維持 |
