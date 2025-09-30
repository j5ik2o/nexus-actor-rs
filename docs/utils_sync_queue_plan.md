# utils Sync Queue 移行メモ (2025-09-29 時点)

## 区分基準
- **完了済みフェーズ**: 実装済みの移行ステップ。
- **残タスク**: 今後の削除・整理項目。
- **留意事項**: 実装／運用上の注意点。

## 完了済みフェーズ
- フェーズ1: `utils/src/collections/queue_sync.rs` に `SyncQueueBase` / `SyncQueueWriter` / `SyncQueueReader` を定義し、`RingQueue`・`MpscBounded`・`MpscUnbounded`・`PriorityQueue` が実装済み。
- フェーズ2: 既存の async トレイト (`QueueBase` / `QueueWriter` / `QueueReader`) が同期版へ委譲する default impl を提供（`utils/src/collections/queue.rs`）。既存呼び出しは `await` 付きでも動作。
- フェーズ3: Core / Remote のホットパスは同期ハンドル経由で利用。`DefaultMailbox` や `EndpointWriterMailbox` が `SyncQueue` API を直接使用。

## 残タスク
- フェーズ4: async トレイトの `#[deprecated]` 化と削除スケジュール策定。`legacy-async-queue` feature の導入を検討。
- `queue_throughput` ベンチを同期 API 版へ更新し、比較のための baseline を取得（現状は async 経由）。
- README / API Docs に同期キュー API の利用例を追加し、移行ガイドを公開する。

## 留意事項
- `SyncQueueSupport` を実装したキューは `Send + Sync` 前提。`MutexGuard` を保持したまま `await` しない設計を維持する。
- 将来的な `no_std` 対応を見据え、`parking_lot` 依存の抽象化が必要になった場合は別タスクで検討する。

