# Mailbox ロック順序メモ (2025-09-29 時点)

## 区分基準
- **ロック階層**: 取得順序と役割。
- **検証済み呼び出し**: 主要メソッドでの実際のロック解放タイミング。
- **注意点**: 新規コード追加時に踏むべきルール。

## ロック階層
1. `DefaultMailbox.inner` (`Arc<Mutex<DefaultMailboxInner<...>>>`): ハンドルやメトリクストラッカーの所有権を読み出すためにのみ取得。
2. `QueueWriterHandle` / `QueueReaderHandle`: `offer_sync` / `poll_sync` の内部で `Mutex<Queue>` を短期間保持。
3. `QueueLatencyTracker.timestamps`: `Mutex<VecDeque<Instant>>` でラッチし、同期 push/pop を実行。

## 検証済み呼び出し
- `DefaultMailbox::poll_system_mailbox` / `poll_user_mailbox`（`modules/actor-core/src/actor/dispatch/mailbox/default_mailbox.rs:430-520`）: `inner` ロック→ハンドル clone→ロック解放→`poll_sync` 実行（`await` 無し）。
- `DefaultMailbox::post_user_message` / `post_system_message`：同様に `inner` ロック後に `offer_sync` を呼び、非同期 `await` は行わない。
- `DefaultMailbox::record_queue_dequeue` 経由のメトリクス更新では、`inner` ロック後に `QueueLatencyTracker` を clone し、ロック解放後に同期操作のみを実施。

## 注意点
- `inner` ロックを保持したまま `await` すること、または `QueueWriterHandle` / `QueueReaderHandle` / `QueueLatencyTracker` のロックとネストすることを禁止。
- 新しいメトリクスフックを追加する場合も「状態読み出し→ロック解放→非同期操作」の順に従う。
- デバッグ時は `parking_lot::deadlock` feature で検出できるため、デッドロック懸念がある変更では必ず有効化して確認する。

