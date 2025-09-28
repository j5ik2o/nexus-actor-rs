# utils Sync Queue 移行計画 (2025-09-26)

## ゴール
- `parking_lot` + `Atomic*` に刷新したキュー実装を、`async_trait` 非依存の同期 API で扱えるようにする。
- 既存 `QueueBase`/`QueueReader` 等を段階的に同期版へ委譲し、互換性を保ちながら最終的に `async` トレイトを廃止する。

## 前提
- 現状の `RingQueue`/`MpscBoundedChannelQueue`/`MpscUnboundedChannelQueue` は内部同期処理の `await` を排除済み。
- 使用側コードは `async fn` を呼び出す前提で書かれているため、まず互換レイヤーで吸収する。
- 将来的に `no_std` 対応を見据えるためにも、同期 API を先に確立しておくと移行しやすい。

## フェーズ分割
### フェーズ 1: 同期トレイトの導入
- 新規に `SyncQueueBase` / `SyncQueueWriter` / `SyncQueueReader` を `utils/src/collections/queue_sync.rs` などに定義。
  - `len` / `capacity` / `offer` / `poll` など同期メソッドのみを提供。
  - デフォルト実装で `is_empty` / `offer_all` など補助関数も用意。
- 既存実装（RingQueue/Mpsc〜）に対して同期トレイトを実装。

### フェーズ 2: 互換ブリッジの実装
- 既存 `QueueBase`/`QueueWriter`/`QueueReader` (async) で、同期版が実装されている場合は同期メソッドを呼び出すようにデフォルト実装を追加。
  ```rust
  #[async_trait]
  pub trait QueueBase<E: Element>: Debug + Send + Sync {
      async fn len(&self) -> QueueSize;
      // ...
  }

  impl<E, T> QueueBase<E> for T
  where
      E: Element,
      T: SyncQueueBase<E> + Debug + Send + Sync,
  {
      async fn len(&self) -> QueueSize {
          SyncQueueBase::len(self)
      }
      // ...
  }
  ```
- 使用側は既存 async API のままでも動作し、同期トレイトを実装していれば自動的に成果が反映される。

### フェーズ 3: 使用箇所の同期 API 置換
- `RingQueue` 等を利用している箇所で、`await` が不要な場面から順次 `SyncQueue*` API に切り替え。
- 例: `queue.offer(e).await?;` → `queue.sync_writer().offer(e)?;` など、用途に応じたラッパを検討。
- 既存の async トレイトを使用しているテスト・サンプルは互換レイヤーのままでも動作するため、影響範囲を見ながら段階的に移行。

### フェーズ 4: async トレイトの段階的廃止
- 同期 API への移行が完了したタイミングで `QueueBase` 等を `#[deprecated]` に指定。フィーチャゲート `legacy-async-queue` の導入も検討。
- 問題なければ最終的に `async` トレイトを削除し、同期 API のみを公開する。

## タスク一覧 (60分自動実行を想定)
1. `queue_sync.rs` など新ファイルで同期トレイト群を定義し、`lib.rs` から re-export。
2. `RingQueue`/`MpscBounded`/`MpscUnbounded` に `SyncQueue*` を実装。
3. `QueueBase`/`QueueWriter`/`QueueReader` に同期実装へ委譲する default impl を追加。
4. 主要テスト・サンプルが現状どの API を使っているか確認し、`await` 依存が無い箇所から同期メソッドへ置換（可能な範囲）。
5. ベンチ (`queue_throughput.rs`) を同期 API のラッパへ置き換え、動作確認 (`cargo check`, `cargo bench --bench queue_throughput`)。
6. ドキュメント (`docs/legacy_examples.md` や README の該当箇所) に同期 API の存在を追記。
7. 残課題: `legacy-async-queue` フィーチャ導入／汎用ラッパの検討（時間に余裕があれば着手）。

## 留意事項
- 同期 API は `Send`/`Sync` を前提に設計するため、`MutexGuard` を保持したまま `await` に進まないよう現状の設計を維持する。
- `no_std` 移行を見据えて、同期トレイト内部で `parking_lot` / `Atomic*` に依存しないよう抽象化する場合は別タスクで検討。
- フェーズ 3 の移行は広範囲に及ぶ可能性があるため、CI での動作確認や段階的マージを前提に進める。

