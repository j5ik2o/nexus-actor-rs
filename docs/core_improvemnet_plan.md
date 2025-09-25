# core ライフタイム移行計画

最終更新: 2025-09-25

## ビジョン
- `ActorContext` / `TypedActorContext` 系の API をライフタイム指向に統一し、`ContextBorrow<'_>` を中心とした参照モデルへ移行する。
- `Arc<Mutex<_>>` 依存を段階的に排除し、`WeakActorSystem` + 借用ビューによる循環参照防止とロック時間の最小化を両立する。
- メトリクス・ベンチマーク・ドキュメントを連携させ、性能退行や API 破壊を即座に検出できる体制を整える。

## ライフタイム移行 3 原則
1. **借用優先**: 共有状態は `ContextBorrow<'_>` か `TypedContext` 経由でアクセスし、構造体への所有権コピーを避ける。
2. **弱参照駆動**: `ActorSystem` や Extension は `WeakActorSystem` / `ContextExtensionHandle` で保持し、ドロップ順序の制御を不要にする。
3. **ロック最小化**: 残存する `Arc<Mutex<_>>` はスコープ短縮・`RwLock` 化・借用化のいずれかで置換し、ホットパスの待ち時間を削減する。

## 現状サマリ
- `ActorContext::borrow` を導入済み。`snapshot()` は互換ラッパーとして維持。
- RootContext, DeadLetterProcess など主要コンポーネントで `WeakActorSystem` を採用し循環参照を解消。
- `docs/typed_context_guidelines.md` / `docs/dispatcher_runtime_policy.md` を整備し、ライフタイム利用とランタイム管理の指針を共有。
- CI: PR ベンチ (`bench.yml`) + 週次ベンチ (`bench-weekly.yml`) で `reentrancy` / `context_borrow` を監視。

## TODOサマリ（2025-09-25 時点）
※ `[ ]` が付いているもののみ未完了タスク。
- **フェーズ B: コンテキスト拡張と Supervisor**
  - [ ] `TypedActorHandle` / `PidActorRef` の弱参照化を完了
- **フェーズ C: 観測と回帰防止**
  - [ ] ベンチ結果を CSV 化して履歴追跡 (`scripts/export_bench_metrics.py`)
  - [ ] `docs/bench_dashboard_plan.md` に沿った GitHub Pages 公開
  - [ ] `cargo make coverage` をライフタイム回帰テストに組み込み
  - [ ] リグレッション用のスプリント別タスク整理と完了チェック運用の自動化
- **同期 API 移行**
  - [ ] `ReceiverContextHandle` / `TypedContextHandle` に同期 borrow API を提供し、`ReceiverPart` 系のホットパスでロックを排除
  - [ ] `SupervisorHandle` を `ArcSwap<dyn Supervisor>` 化し、`SupervisorStrategy` から同期 borrow を可能にする
  - [ ] `async_trait` 依存を棚卸しし、同期化できるメソッドを通常メソッドへ置換
  - [ ] protoactor-go (`actor/context.go`, `actor/supervision.go`) の呼び出し順序差分を `docs/dispatcher_runtime_policy.md` へ反映するドラフトを作成
  - [ ] TypedActorContext 系の同期化済み borrow に合わせて情報取得メソッドを再整理する
- **計測とスナップショット運用**
  - [ ] ReceiverMiddlewareChain 実行部で `context_cell_stats()` の差分をログ収集する
  - [ ] `SupervisorHandle::new(self.clone())` など既存生成箇所に `inject_snapshot()` を組み込み、初期化直後からスナップショットが利用されるか確認する

## ロードマップ詳細
### フェーズ A: ActorContext コアの再編
- ✅ `ContextBorrow<'_>` による props/self 参照の提供
- ✅ `WeakActorSystem` 化と循環参照の除去
- ✅ `ActorContext` 内部ロックの分離 (`extras` / `message_or_envelope`)
- ✅ 初期ベンチ (`context_borrow`) の追加
- ✅ `ActorContextShared` を `OnceCell<ArcSwapOption<ActorHandle>>` ベースに刷新し、アクター差し替え時のロックを排除

### フェーズ B: コンテキスト拡張と Supervisor
**完了済み**
- ✅ Extension 管理 (`ctxext::extensions`) の `Arc<Mutex<Vec<_>>>` を `RwLock` + スライス借用に移行
- ✅ Supervisor 階層の `Arc<Mutex<dyn Supervisor>>` を `Arc<RwLock<dyn Supervisor>>` に刷新し、再入ロックを削減

**残タスク（TODOサマリ参照）**
- ⏳ `TypedActorHandle` / `PidActorRef` の弱参照化を完了

### フェーズ C: 観測と回帰防止
**完了済み**
- ✅ `scripts/check_context_borrow_bench.sh` を CI へ統合

**残タスク（TODOサマリ参照）**
- ⏳ ベンチ結果を CSV 化して履歴追跡 (`scripts/export_bench_metrics.py`)
- ⏳ `docs/bench_dashboard_plan.md` に沿った GitHub Pages 公開
- ⏳ `cargo make coverage` をライフタイム回帰テストに組み込み
- ⏳ リグレッション用のスプリント別タスク整理と完了チェック運用の自動化

## スプリント実績とログ
### Sprint 3 実績
- ✅ `Arc<Mutex<_>>` 使用箇所の棚卸し (`rg "Arc<Mutex" core/`) を自動化し、タスクリストを `docs/typed_context_guidelines.md` に追記 (`scripts/list_arc_mutex_usage.sh`)
- ✅ `ActorContextShared` の `ArcSwapOption` 版に合わせ、`ContextBorrow` からのアクター参照 API を整理（同期化 + 再起動整合性確認, 2025-09-25）
- ✅ Extension レイヤの borrow API (`borrow_extension` / `borrow_extension_mut`) を実装し、`ContextExtensionHandle` の clone を削減

### Sprint 4 実績
- ✅ Supervisors/metrics 経路の async 呼び出しを棚卸しし、`record_supervisor_metrics` で同期パスへ統合 (2025-09-25)
- ✅ ContextHandle の `Arc<Mutex>` 利用箇所を `ArcSwap` 化する PoC を実装し、`RwLock` 維持で受信順序を確認 (2025-09-25)
- ✅ Extension レイヤの borrow API 設計を Sprint 3 成果に統合済み

#### Sprint 4 詳細タスクメモ
- ✅ **Supervisor/metrics 同期化設計案**: `ActorSystem::metrics_foreach` / `Metrics::foreach` の同期版を整備し、`ActorContext::metrics_sink()` と Supervisor 戦略を共通ヘルパー `record_supervisor_metrics` に統合（2025-09-25）
- ✅ **ContextHandle ArcSwap PoC**: `ContextHandle` 内部を `ArcSwap<RwLock<Box<dyn Context>>>` ベースへ刷新し、`ContextCell` スナップショット併用でホットパスの `Arc` クローンを削減（2025-09-25）
- 📝 **実装優先のレビュー準備**: 破壊的変更となる API 差分を洗い出し、`core/actor/context` 配下のタスクリストを Sprint 4 終了時点で即実装に入れるよう整理する（継続メモ）

### Sprint 4 調査ログ
- Supervisors/metrics: `ActorSystem` に `ArcSwapOption<MetricsRuntime>` を導入し、同期クロージャで `metrics_foreach` を実行可能にする PoC を実装。`ActorContext::metrics_sink()` は同期キャッシュ初期化を行うよう更新済み。Supervisor 戦略（`one_for_one` / `all_for_one` / `restarting`）は `record_supervisor_metrics` を通じて `ActorSystem::metrics_foreach` を利用し、`SupervisorHandle::metrics_sink` 依存を除去。今後は `ContextBorrow` / `ReceiverContext` 周りを同期 API へ拡張する。
  - ContextHandle: メッセージセル (`message_or_envelope_opt`) の整合性確保のため `Arc<Mutex>` が引き続き必要。`ArcSwap` 化後はキュー投入順序と stashing を別構造へ退避する PoC を検討する。
  - ContextHandle: `ArcSwap<RwLock<Box<dyn Context>>>` による PoC を実装済み。`MessagePart` など async API 依存は残るが、ホットパスのスナップショット取得が同期化され、受信順序は `RwLock` 維持で保証される。

#### ContextHandle / Supervisor API 差分（2025-09-25 調査）
- `ContextHandle` は `Arc<RwLock<dyn Context>>` を保持しつつ、`ContextCell` 経由で `ActorContext` のスナップショットを `ArcSwapOption` で公開するよう改修（`core/src/actor/context/context_handle.rs:24-212`）。protoactor-go と同様に借用中のロック保持を避けるには、`ContextCell` の read/write 双方を活用する追加改修が必要。
- `ReceiverContextHandle` / `TypedContextHandle` も `Arc<RwLock>` ベースで async を前提としている（例: `core/src/actor/context/receiver_context_handle.rs:15-83`）。`ContextHandle` 差し替えと合わせて同期 API へ刷新する必要がある。
- `SupervisorHandle` は `Arc<RwLock<dyn Supervisor>>` に依存し、`borrow()` が async（`core/src/actor/supervisor/supervisor_strategy.rs:71-165`）。protoactor-go の `supervision.go` に倣い、`ArcSwap<dyn Supervisor>` + 即時 borrow を検討する。
- Supervisor イベント購読では `subscribe_supervision` が `ActorSystem::get_event_stream()` を await（`core/src/actor/supervisor/supervision_event.rs:18-34`）。同期化に合わせてイベント発火側の設計を見直す必要がある。

##### 2025-09-25 実装メモ
- ReceiverContextHandle: `ContextHandle` を内包し、`actor_context_arc()` で `ContextCell` スナップショットを同期取得する PoC を追加（`core/src/actor/context/receiver_context_handle.rs:16-92`）。
- TypedContextHandle: `actor_context_arc()` を公開し、Typed 経路でも同期スナップショットを参照可能に。
- SupervisorHandle: `SupervisorCell` を導入し、`ArcSwapOption<dyn Supervisor>` ベースのスナップショット API (`supervisor_arc`) を提供（`core/src/actor/supervisor/supervisor_strategy.rs:86-180`）。今後は RwLock 除去に向けてセル更新タイミングを整理する。
- 測定手段: `ContextCellStats` / `SupervisorCellStats` を追加し、`context_cell_stats()` / `supervisor_cell_stats()` からヒット数・ミス数を取得可能。ホットパス実行後に差分を取ることで同期参照化の効果と未カバー領域を定量化できる。
- `SupervisorHandle::inject_snapshot` を追加し、`SupervisorHandle::new_arc` など RwLock ベース生成後でも任意タイミングでスナップショットを補足可能にした。`GuardianProcess` 等の構築時に Clone 済みインスタンスを渡すことでセルを即同期化できる。

## 実装タスクリスト（優先度順ドラフト）
**完了済み**
- ✅ `ContextHandle` を `ArcSwap` ベースへ刷新するための `ContextCell`（仮）を追加し、`ContextBorrow<'_>` と統合する PoC を実装（対象: `core/src/actor/context/context_handle.rs`, `core/src/actor/context/actor_context.rs`）

**TODO（TODOサマリ参照）**
- ⏳ `ReceiverContextHandle` / `TypedContextHandle` に同期 borrow API を提供し、`ReceiverPart` などの高速パスでロックを取らないようにする（対象: `core/src/actor/context/receiver_context_handle.rs`, `core/src/actor/context/typed_context_handle.rs`）
- ⏳ `SupervisorHandle` を `ArcSwap<dyn Supervisor>` 化し、`SupervisorStrategy` 実装から同期 borrow が使えるよう `SupervisorCell` を導入する（対象: `core/src/actor/supervisor/supervisor_strategy.rs`, `core/src/actor/supervisor/supervisor_strategy_handle.rs`）
- ⏳ 上記に伴う `async_trait` 依存の見直しを実施し、同期化できるメソッドを通常メソッドへ変更する。変更後は `tokio::sync::RwLock` を削除し、`ContextBorrow` ベースのメトリクス経路が成立するか確認する。
- ⏳ protoactor-go (`actor/context.go`, `actor/supervision.go`) の呼び出し順序を写経し、所有権管理やライフタイム差分を `docs/dispatcher_runtime_policy.md` へ反映するドラフトを作成する。

## Sprint 5 プランニング (案)
- **ArcSwap ベース ContextHandle の本実装**: Sprint 4 PoC のフィードバックを取り込み、`ContextHandle` / `ActorCell` 周辺の API を再設計。protoactor-go の `SharedContext` / `ActorCell` に倣い、borrow 可能なメッセージセルを `Cow` でラップする案を検証。
- **Supervisor Metrics の段階的同期化**: `metrics_foreach` の非同期依存を `ArcSwap<MetricsRegistry>` 経由の同期アクセスへ移行し、OpenTelemetry exporter を別タスクへ切り出す。影響範囲を `docs/dispatcher_runtime_policy.md` と `docs/typed_context_guidelines.md` に反映。
- **回帰テスト拡充**: `cargo test -p core actor::context::tests::` に加え、`loom` を用いた並行試験のドラフトを準備。`protoactor-go/tests/context_test.go` を参照しつつ Rust 版の競合再現シナリオを作成する。

### Sprint 5 調査ログ（ドラフト）
- ArcSwap 化により `ReceiverContext` でのメッセージ所有権移譲が複雑化する恐れがある。`ContextBorrow<'_>` の lifetime を `tokio::task::LocalSet` へ束縛する案を検証予定。
- Supervisor メトリクス同期化では `MetricsContext` の exporter 初期化順序がボトルネック。`WeakActorSystem` から exporter を取得する際の再入を避けるため、初期化専用タスク追加の要否を評価する。
- ベンチダッシュボードや CSV 連携はバックログへ退避。実装安定後に再検討し、それまでは `bench.yml` / `bench-weekly.yml` の閾値監視で運用する。

## 運用ルール
- 本計画書を更新した際は、完了済み項目のチェックやスプリントの次アクションを必ず見直す。

### async -> sync 変換候補（メモ）
**TODO（TODOサマリ参照）**
- ⏳ TypedActorContext 系は borrow を同期化済み。情報取得メソッド群は underlying `ActorContext` の async を再利用しているため、Supervisor/metrics の同期化後に再整理する。

**完了済み / 進捗メモ**
- ✅ ContextHandle 系の `Arc<Mutex>` はメッセージセル制御に依存しており、`ArcSwap` 化 PoC を実装しつつ `RwLock` 維持で順序保証を確認。
- ContextBorrow 内のメトリクス経路は `ArcSwapOption<MetricsRuntime>` を参照する PoC に更新済み。`metrics_sink()` 経由で await を挟まない取得が可能になったため、Supervisor 側の呼び出し線図を同期化した後に最終的な API 置換を実施する予定。
- ✅ `ActorHandle::type_name`: 型名取得のための `await` を `Arc<str>` キャッシュ化で同期メソッドに変更（2025-09-25）
- ✅ `ActorContext::get_actor` / `set_actor`: `ArcSwapOption` 化により同期アクセサへ置換済み（2025-09-25）
- ✅ `ActorContext::borrow`: メトリクス参照を除き同期化完了。`self_pid` を `RwLock` から `ArcSwapOption` へ移行する案を検討中。
- `TypedActorContext::borrow`: 同期版 `ActorContext::borrow` に合わせて単なるラッパーへ変更予定。
- `ContextHandle` 系 `get_message_handle`: 既存ロック再利用のため非同期維持。差し替え難度が高いため保留。

## 計測と検証
- テスト: `cargo test --workspace`
- ベンチ: `cargo bench -p nexus-actor-bench --bench reentrancy`
- 閾値: CI では `REENTRANCY_THRESHOLD_NS` と `CONTEXT_BORROW_THRESHOLD_MS` をリポジトリ変数で管理。
- ダッシュボード/CSV 化はバックログ: 実装タスク完了を優先し、`docs/bench_dashboard_plan.md` のワークフロー整備は後続スプリントで再開予定。

**TODO（TODOサマリ参照）**
- ⏳ ReceiverMiddlewareChain 実行部で `context_cell_stats()` の差分をログ出力し、実際のヒット率を収集する。
- ⏳ `SupervisorHandle::new(self.clone())` など既存生成箇所に `inject_snapshot()` を組み込み、初期化直後からスナップショットが利用されるか確認する。

## リスクと対策
- **性能退行**: ライフタイム化に伴う同期手段の変更でベンチ数値が悪化する可能性。→ 変更毎に `scripts/check_*_bench.sh` を実行し、CI のアラートを必ず確認。
- **API 破壊**: `ContextBorrow` 導入に伴い旧 API の互換性が失われる恐れ。→ 破壊的変更は README / CHANGELOG へ明示し、protoactor-go 読み替え指針を添付。
- **テスト不足**: ライフタイム関連の退行は静的解析で検知しづらい。→ `cargo miri` や `loom` を用いた追加検証を検討。

## 参考ドキュメント
- `docs/typed_context_guidelines.md`
- `docs/dispatcher_runtime_policy.md`
- `docs/bench_dashboard_plan.md`
- protoactor-go: `@docs/sources/protoactor-go`（Go 実装をライフタイム指向に読み替える際の参照）
