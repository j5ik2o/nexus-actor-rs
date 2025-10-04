# DefaultMailbox::run メモ (2025-09-29 時点)

## 区分基準
- **現状**: 実装済みの処理フローと確認ポイント。
- **残課題**: 今後検討すべき改善アイデア。

## 現状
- `should_yield` は Dispatcher からのヒントと backlog 感度を考慮し、`modules/actor-core/src/actor/dispatch/mailbox/default_mailbox.rs:620-660` で `dispatcher_hint` を受け取る実装へ更新済み。
- System/User キュー処理は `try_handle_system_message` / `try_handle_user_message` に分離され、queue latency メトリクス更新→Deque→Middleware 呼び出しの順を統一（`同ファイル:660-760`）。
- Suspend/Resume は `AtomicBool` とメトリクス (`MailboxSuspensionMetrics`) で記録され、Resume 時に `record_mailbox_suspension_metrics` を発火する。
- Queue latency / length はサンプリング間引き (`queue_latency_snapshot_interval`) に基づき `MessageInvoker` へ送信。バックログゼロ時はキュー長を即時ゼロ化する。

## 残課題
- System/User のフェアネス制御を数値化し、`queue_kind` ごとの優先比率またはダイナミック優先度切り替えを導入する（現在は backlog のみ参照）。
- Suspend が連続発生するケースのバックプレッシャー挙動（短時間 sleep、強制 resume など）を仕様化し、`MailboxSuspensionMetrics` と連動させる。
- Dispatcher からの過負荷シグナルを拡張し、Mailbox 側で `throughput` 動的調整を行う案を再検討する。
- System/User 混在や大量 Suspend を想定した統合テスト・ベンチ（`mailbox_throughput` 拡張）を追加し、挙動を検証する。

