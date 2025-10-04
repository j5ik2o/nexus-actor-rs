# utils Sync Queue 移行メモ (2025-10-01 時点)

## 区分基準
- **完了済みフェーズ**: 着手順（フェーズ番号）の昇順で整理。
- **残タスク**: 優先度（高→低）でソート。
- **留意事項**: 設計原則ごとに網羅（排他）となるよう分類。

## 完了済みフェーズ（フェーズ番号順）
- フェーズ1: `utils/src/collections/queue.rs` に `QueueBase` / `QueueWriter` / `QueueReader` を定義し、`RingQueue`・`MpscBounded`・`MpscUnbounded`・`PriorityQueue` が実装済み。
- フェーズ2: 既存の async トレイト（旧 `QueueBase` / `QueueWriter` / `QueueReader`） が同期版へ委譲する default impl を提供（`utils/src/collections/queue.rs`）。既存呼び出しは `await` 付きでも動作。
- フェーズ3: Core / Remote のホットパスは同期ハンドル経由で利用。`DefaultMailbox` や `EndpointWriterMailbox` が `Queue` API を直接使用。
- フェーズ4: `modules/utils-std/src/collections/queue.rs` を削除し同期 API に一本化。テストおよび `actor` ベンチから async ラッパーを排除済み。

## 残タスク（優先度高→低）
- [高] README / API Docs に同期キュー API の利用例と移行ガイドを追加し、公開 API 変更点を周知する。
- [中] `legacy-async-queue` 相当の互換機能を提供するか最終判断（不要なら CHANGELOG でサポート終了を明記）。
- [低] `queue_throughput` ベンチで同期版とレガシー版のベースラインを再計測し、ダッシュボードへ反映する。

## 留意事項（設計原則別）
- `QueueSupport` を実装したキューは `Send + Sync` 前提。`MutexGuard` を保持したまま `await` しない設計を維持する。
- 将来的な `no_std` 対応を見据え、`parking_lot` 依存の抽象化が必要になった場合は別タスクで検討する。
