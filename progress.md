2025-09-26 21:37 JST 開始: docs/core_optimization_plan.md のタスクを洗い出し、作業順序を決定。
2025-09-26 21:47 JST 進捗: tokio-console feature と計測用サンプル(actor-context-lock-tracing)を追加し、ActorContextExtras ロック待ち計測のトレースを実装。
2025-09-26 22:05 JST 進捗: PidSet を同期ロック化し、ActorContextExtras の内訳を mutable/read-only 分離。remote endpoint_watcher など呼び出し側も同期 API に更新。
2025-09-26 22:15 JST 進捗: DelayQueue PoC を追加しベンチ記録を更新、MessageHandles を同期スタック化、RestartStatistics を OnceCell 化。README に新セクションを追記し cargo test --workspace を実行。
2025-09-26 22:19 JST 再開: 残タスク（ディスパッチャ経路メトリクス整備・remote/cluster影響ドラフト）に着手。
2025-09-26 22:40 JST 進捗: dispatcher メトリクス設計メモを追加し、actor_context_lock ベンチにキュー滞留計測処理を実装。remote 影響メモも更新済み。
2025-09-26 22:42 JST 進捗: actor_context_lock ベンチを計測ランに通し、サンプルサイズを調整。cargo test --workspace で全クレートのテストを再確認。
2025-10-06 14:30 JST 進捗: Mailbox 抽象を QueueMailbox + Flag で再構築。utils-core に Flag を追加し、actor-core の QueueMailbox が close/disconnect を graceful に処理できるよう修正。テスト queue_mailbox_handles_close_and_disconnect を追加。
2025-10-06 14:45 JST 進捗: embedded_arc 向けに Embassy Signal を使う ArcMailbox を実装。actor-embedded のプレリュードへ再輸出し、cargo test --workspace を実行。
2025-10-06 15:00 JST 進捗: Mailbox 関連ソースを local_mailbox / arc_mailbox / tokio_mailbox へリネームし、モジュール構成を整理。テスト再確認済み。
2025-10-06 16:05 JST 進捗: Mailbox ランタイム抽象化の差分整理と設計メモ (docs/design/2025-10-06-mailbox-runtime-plan.md) を作成。std/embedded 共通化の段階的リファクタリング計画を策定。
2025-10-06 17:20 JST 進捗: MailboxRuntime トレイトと MailboxOptions を actor-core に実装。Tokio/Embedded/Local Mailbox を新抽象へ移行し、Future ラッパや send ハンドラを共有化。LocalMailbox は LocalQueue で Rc backend を共有。cargo test --workspace および no_std arc 構成テストを実行。
2025-10-06 17:45 JST 進捗: embedded_arc ビルド課題の整理、MailboxRuntime 呼び出し箇所と優先度付き Mailbox 拡張計画を設計メモに追記。
2025-10-06 18:15 JST 進捗: ArcMailboxRuntime 用の SignalFuture 依存を除去し、ArcMpsc* キューに Clone 実装を追加。`cargo test -p nexus-actor-embedded-rs --features embedded_arc` を含む指定テスト群を再実行し全て成功。
2025-10-07 10:03 JST 進捗: TokioPriorityMailbox を PriorityEnvelope ベースで再実装し、VecDeque 層の優先度制御と容量制御テストを追加。TokioMailbox の NotifySignal を公開範囲へ変更し、workspace 全体と embedded arc feature のテストを再実行。
2025-10-07 10:26 JST 進捗: ArcPriorityMailboxRuntime を追加し embedded_arc feature 向けの優先度 Mailbox を提供。actor-core に ActorContext / Supervisor / PriorityScheduler を導入し、QueueMailbox からメッセージを排出後に優先度でソートする統合テストを整備。TestMailboxRuntime を用いたスケジューラ検証も実行し cargo test --workspace を再確認。
2025-10-07 10:39 JST 進捗: 優先度付き Mailbox を制御キュー＋通常キューの二層構成へ拡張。Tokio/embedded 双方で新しい複合キュー実装を導入し、MailboxOptions に priority_capacity を追加。制御メッセージ優先処理と容量分離テスト（std/embedded）を整備し、全ワークスペーステストを再実行。
2025-10-07 10:52 JST 進捗: PriorityEnvelope にチャネル種別を導入し、制御メッセージ API（send_control/try_send_control）を追加。ActorContext へ spawn_child / spawn_control_child を実装し、PriorityScheduler が生成された子アクターを次サイクルで処理できるよう更新。Supervisor テストを拡張し、全ターゲットのテストを再実行。
2025-10-07 11:00 JST 進捗: SystemMessage 列挙と from_system ヘルパーを追加し、PriorityEnvelope::map でメッセージ型変換を簡素化。PriorityActorRef の動作テストに制御メッセージケースを組み込み、protoactor-go の SystemMessage 優先度が保持されることを確認。
2025-10-07 11:10 JST 進捗: PriorityActorRef<SystemMessage> に try_send_system を追加し、scheduler の統合テストで制御メッセージ経路を検証。プロトアクターの SystemMessage 送出フローを模した回帰テストを整備。
2025-10-07 11:35 JST 進捗: Guardian と PriorityScheduler を統合し、map_system クロージャ経由で SystemMessage を型付きメッセージへ橋渡し。ActorContext/ChildSpawnSpec に map_system を保持させ、panic 時の guardian 通知と Restart/Stop シナリオをテスト（std feature 付きで panic 捕捉、no_std は早期 return）。scheduler の優先度テストを Message enum ベースへ更新し、新規ガーディアン再起動テストを追加。`cargo test -p nexus-actor-core-rs` および `--features std` を実行。
