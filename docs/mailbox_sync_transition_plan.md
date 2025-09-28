# DefaultMailbox 同期化ロードマップ (2025-09-28)

## 背景
- Mailbox のホットパスはキュー操作・メトリクス記録・スケジューリング判定など短時間で完結する処理が多い。
- 現状は `tokio::Mutex` と `async_trait` を多用しており、`await` 境界で `Future` が頻繁に生成されるためレイテンシと CPU オーバーヘッドが発生している。
- utils 側の Queue 実装を同期 API に刷新したが、Mailbox 側が非同期のままでは恩恵を得にくい。

## ゴール
1. DefaultMailbox（および関連構造）を同期キューベースへ移行し、ホットパスから不要な `await` を排除する。
2. 既存 API を保ちつつ段階的に移行できるブリッジを提供し、remote/cluster など下流モジュールの影響を最小化する。
3. メトリクス・スケジューリング挙動・サスペンド/レジューム処理の正しさを維持するため、単体テストとベンチ強化を行う。

## 前提・制約
- `Mailbox` トレイトは `async_trait` を利用しているため、同期化には呼び出し側のランタイム境界を明示する必要がある。
- `Dispatcher` は `tokio` ランタイム上でスケジュールされるため、Mailbox の同期化は「内部ロックが同期化される」だけでよく、外部公開 API は当面 `async fn` を維持する。
- 既存ハンドル (`QueueWriterHandle`/`QueueReaderHandle`) は `Arc<tokio::Mutex<_>>` をラップしている。ここを `parking_lot::Mutex` へ切り替えるだけでは `await` を除去できないため、ハンドル層の API を同期化しラッパを用意する必要がある。

## フェーズ別タスク

### フェーズ 1: 設計・下準備 (目標: 1〜2 日)
- [ ] DefaultMailbox の責務整理
  - `QueueLayer`: メールボックス毎のキュー操作（enqueue/dequeue、キャパシティ管理）
  - `MetricsLayer`: キュー滞留時間やカウンタの更新
  - `SchedulerLayer`: Dispatcher との連携とスケジューリング制御
  - `ControlLayer`: Suspend/Resume や Failure Escalation など制御フロー
- [ ] ロック順序の明文化（暫定案）

| 順序 | ロック対象 | 目的 |
| ---- | ---------- | ---- |
| L1 | `DefaultMailbox.inner` (mutable state) | 中心ステート更新・ハンドル複製 |
| L2 | `QueueLatencyTracker.timestamps` | メトリクス push/pop |
| L3 | `SyncQueueWriterHandle` / `SyncQueueReaderHandle` 内部キュー | enqueue/dequeue |
- L1 を保持したまま L2/L3 を取得しない（逆順取得を禁止）方針をマニュアル化し、deadlock を回避する。
- [ ] DefaultMailbox 内部状態の責務分解 (Queue 状態管理、メトリクス、スケジューリング) を整理し、同期化済みの構造に再編するための UML/表形式ドキュメントを作成。
- [ ] Queue ハンドル層を書き換える方針を決定：
  - `SyncQueueWriter`/`SyncQueueReader` を直接保持する同期ハンドル (`SyncQueueWriterHandle` 等) を追加。
  - 既存の非同期ハンドルは薄いアダプタとして残し、利用側の API を壊さずに段階移行。
- [ ] `QueueLatencyTracker` を `parking_lot::Mutex<VecDeque<Instant>>` へ変更する PoC を作成し、副作用を評価。

### フェーズ 2: ハンドル層の同期化 (目標: 2〜3 日)
- [x] `DefaultMailbox` での差し替え手順整理
  1. `QueueWriterHandle`/`QueueReaderHandle` を `SyncMailboxQueueHandles` で wrap した構成にリネームし、`parking_lot::Mutex` を利用。
  2. `DefaultMailboxInner` にユーザー／システム双方の `SyncQueueWriterHandle` / `SyncQueueReaderHandle` を保持させ、既存の async ベース実装を削除。
  3. `DefaultMailbox::new` で `SyncMailboxQueueHandles::new(queue)` を生成し、reader/writer ハンドルを取得して `DefaultMailboxInner` へ格納。
  4. メールボックス経路で呼び出している `offer` / `poll` / `clean_up` を同期メソッド経由に切り替え、`async fn` は薄い互換レイヤーのまま維持。
- [x] ディスパッチャ／テストへの影響洗い出し
  - `MailboxProducer` / `BoundedMailboxQueue` / `UnboundedMailboxQueue` から渡されるキューが `SyncQueueSupport` を満たしているか確認。
  - `mailbox/tests.rs` などハーネスコードが `await` している箇所を抽出し、同期メソッドに置き換えた場合の動作確認手順を列挙。
- [x] `QueueWriterHandle`/`QueueReaderHandle` を同期バージョンに差し替え、内部キュー操作を同期メソッド (`offer_sync`/`poll_sync`) に統一。
- [x] `BoundedMailboxQueue` / `UnboundedMailboxQueue` を `SyncMailboxQueue` 実装へ移行し、`DefaultMailbox` のジェネリクス境界に適合。
- [x] Mailbox 内部でのキュー呼び出しを同期メソッドに更新し、ロック保持中の `await` を排除。
- [x] `SyncQueueSupport` を満たすキューのみを受け取るよう型境界を更新。

- [x] `DefaultMailboxInner` のロックを `parking_lot::Mutex` へ切り替え、頻繁に呼ばれるメソッド (`increment_user_messages_count` 等) を同期メソッドへ変換。
- [ ] `run` ループを再構成：
  - [x] system/user ハンドリングをヘルパ化し、ロック解放後に `await` する構造へ整理。
  - [x] `tokio::task::yield_now` のトリガ条件と Dispatcher 連携を再設計（フェアネス／バックプレッシャー検証を含む）。
    - `DefaultMailbox::should_yield` が `dispatcher.yield_hint()` と `MailboxYieldConfig`（`system_burst_limit` / `user_priority_ratio` / `backlog_sensitivity`）を反映するよう更新し、hint が連続発火した際の starvation を避けるため `iteration > 0` ガードを追加。
    - [x] Dispatcher の `yield_hint` が常時 `true` の環境でも Mailbox が前進することを保証する統合テスト（`test_mailbox_progress_with_dispatcher_yield_hint`）を追加。
  - [x] メールボックス停止・レジュームの判断に同期データ構造を利用。
    - `MailboxSuspensionState` を導入し、`AtomicBool` + `Mutex<Option<Instant>>` で状態と経過時間を保持。`DefaultMailbox` からは同期メソッドで参照するように変更し、`run` ループ内の `await` を削減。
- [x] メトリクス記録 (`record_queue_enqueue/dequeue`) を同期化し、Tokio の `Instant` を維持。
  - `QueueLatencyTracker` が `std::time::Instant` ベースで動作することを確認し、非同期ロックを排除。
  - Mailbox 停止・再開時の累積時間と発生回数を `MailboxSuspensionMetrics` で収集し、`DefaultMailbox::suspension_metrics` から参照可能にした。
  - `LatencyHistogram` を導入し、`DefaultMailbox::queue_latency_metrics()` から user/system キューのパーセンタイルを取得できるようにした。
  - `ActorMetrics` に `nexus_actor_mailbox_queue_dwell_percentile_seconds` Gauge を追加し、`p50`/`p95`/`p99` を queue_kind 属性付きで OTEL へ記録。
  - TODO: queue 滞留時間のヒストグラム集計や suspend/resume イベントのメトリクス化を検討。

### フェーズ 4: API 整理と後片付け (目標: 2 日)
- [ ] Mailbox トレイトを二層構造に分けることを検討：
  - 内部同期 API (`MailboxSync`) を追加し、既存の非同期 `Mailbox` は薄いアダプタとして残す。
- [ ] remote/cluster/tests を同期 API に合わせて改修。
- [ ] ベンチを追加 (`bench/benches/mailbox_throughput.rs` 等) し同期化後の性能を測定。
- [ ] ドキュメント更新（本ファイルに進捗追記、`docs/core_optimization_plan.md` の Mailbox 項を更新）。

## 検証計画
- フェーズごとに `cargo check --workspace` と `cargo test --workspace` を必須チェックとする。
- Mailbox 関連の割り込みケース（サスペンド/レジューム、優先度付きキュー、システムメッセージ）を網羅する統合テストを追加。
- 既存ベンチ `queue_throughput` に加え、Mailbox 全体を計測する Criterion ベンチを新設。

## リスクと緩和策
- **同期ロック化によるデッドロックリスク**: 呼び出し順序を明示した設計ドキュメントを用意し、コードレビューで確認。
- **Tokio ランタイムとの互換性**: スケジューラへ制御を戻すポイントを保持し、`Dispatcher` とのインターフェイス変更を最小化。
- **ベンチ結果の劣化**: 各フェーズでベンチを取り、性能が悪化した場合は即座にロールバックまたは代替案を検討。

## 次アクション (2025-09-28 時点)
1. Dispatcher `yield_hint` 連携後の挙動を `mailbox_throughput` ベンチと統合テストで確認し、過剰な `tokio::task::yield_now` 発火がないか監視する。
2. `docs/mailbox_lock_order.md` を基にレビュー観点を共有。
3. Mailbox ベンチ結果（スループット測定）を継続収集し、最適化施策の評価指標とする。

## ベンチ結果 (2025-09-28)
- 実行コマンド: `cargo bench --bench mailbox_throughput`
- 計測対象: `DefaultMailbox` + unbounded MPSC メールボックス
- 最新結果:
  - `mailbox_process/unbounded_mpsc_100`
    - time: 117.39 µs – 124.54 µs
    - throughput: 802.97 Kelem/s – 851.87 Kelem/s
  - `mailbox_process/unbounded_mpsc_1000`
    - time: 1.0318 ms – 1.0770 ms
    - throughput: 928.51 Kelem/s – 969.21 Kelem/s
- 備考: Dispatcher `yield_hint` を考慮するロジック導入後の再計測（Tokio デフォルト Dispatcher は現時点で `false` を返すため実質的には構成値の影響のみ）。`unbounded_mpsc_100` は前回計測 (111.50–122.47 µs) から軽微な劣化範囲内で変動しつつ統計的には有意に改善、`unbounded_mpsc_1000` は誤差範囲で横ばい。

### 参考: 同期化前コミット (`0ae465fc`)
- 条件: `bench/benches/mailbox_throughput.rs` を手動追加して実行
- 結果:
  - `mailbox_process/unbounded_mpsc_100`
    - time: 3.7656 ms – 3.9155 ms
    - throughput: 25.539 Kelem/s – 26.556 Kelem/s
  - `mailbox_process/unbounded_mpsc_1000`
    - time: 35.796 ms – 39.212 ms
    - throughput: 25.502 Kelem/s – 27.936 Kelem/s

次ステップでは、同期化前コミットでの測定と `run` ループ再構成後の再計測を予定。

進捗はこのファイルに追記し、完了タスクを `- [x]` へ反映すること。
