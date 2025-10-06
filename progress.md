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
