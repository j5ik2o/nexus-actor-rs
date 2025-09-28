# Mailbox ロック順序メモ (2025-09-28)

## 1. ロック対象
- `DefaultMailbox.inner`: `Mutex<DefaultMailboxInner<..>>`
- `DefaultMailboxInner.user_mailbox_* / system_mailbox_*`: `SyncQueue*Handle` (内部 `Mutex<Queue>`)
- `QueueLatencyTracker.timestamps`: `Mutex<VecDeque<Instant>>`

## 2. 取得パターン
1. `DefaultMailbox.inner` → clone handles / trackers → lock解放 → 非同期操作 (`await`) 実行
2. `SyncQueue*Handle` → 同期 `offer_sync` / `poll_sync`
3. `QueueLatencyTracker.timestamps` → 同期 push/pop/clear

## 3. 禁止事項
- `DefaultMailbox.inner` を保持したまま `SyncQueue*Handle` や `QueueLatencyTracker` をロックしない。
- `DefaultMailbox.inner` を保持したまま `await` しない。

## 4. 現状のコード確認
- `poll_system_mailbox` / `poll_user_mailbox`: `inner` ロック → handle clone → ロック解放 → `await`
- `offer_*`: 同上
- メトリクス (`record_queue_*`): `inner` ロック → tracker clone → ロック解放 → 同期処理
- スケジューラ (`compare_exchange_scheduler_status` など): `inner` ロックのみ、`await` 無し

## 5. 今後の注意点
- 新しいメソッドを追加する際は「ロック取得→必要情報複製→ロック解放→非同期処理」の順序を守る。
- `SyncQueue*Handle` 内部の `Mutex<Q>` 取得後は同期呼び出しのみを行う。`await` を導入しない。
- デバッグ時は `parking_lot::deadlock` feature で検知可能。

