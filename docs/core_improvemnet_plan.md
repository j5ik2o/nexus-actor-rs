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

## TODOサマリ（2025-09-26 時点）

### 未完了タスク
- [x] **ContextHandle の ArcSwap 本実装** - PoC をプロダクション品質に引き上げ、ContextBorrow<'_> のライフタイム境界を明確化する。ReceiverContext 系で所有権移譲が複雑化するリスクに対する対策も含める。
- [x] **Supervisor Metrics の完全同期化** - record_supervisor_metrics 経路を ArcSwap<MetricsRuntime> 経由で同期アクセス化し、exporter 初期化順のボトルネックを解消してライフタイム再入リスクを下げる。
- [x] **Concurrent Test（loom 等）の整備** - 同期アクセス設計の安全性を loom などで検証し、並行シナリオで予期しない await が混在しないか確認する。
- [x] ベンチ結果を CSV 化して履歴追跡 (`scripts/export_bench_metrics.py`)
- [x] `docs/bench_dashboard_plan.md` に沿った GitHub Pages 公開（bench-weekly ワークフロー & Pages ダッシュボードを追加）
- [x] `cargo make coverage` をライフタイム回帰テストに組み込み（`cargo make lifetime-regression` タスクを追加）
- [x] **ContextBorrow ホットパスのロック計測** - context_borrow/borrow_hot_path ベンチの軽度な退行を調査するため、ContextHandle 経路のロック取得頻度を計測し、プロファイラでホットパスを再確認する。同期 getter 再設計メモに計測ログを反映済み。
- [x] **RootContext/SenderContext の同期アクセサ再設計** - RootContext::request_future 経路を含む送信系 API から非同期ロックを排除し、guardian/metrics 取得も ArcSwap スナップショット化する。SenderContextHandle は ContextHandle/RootContext の同期参照のみを許可し、pipeline 経路で await を撤廃する。**完了 (2025-09-26)**: RootSendPipeline/RootRequestFuturePipeline を同期化、ActorSystem::guardians_snapshot 追加、SenderContextHandle で try_get_sender_opt 利用
- [x] **ContextHandle メッセージセル刷新** - message_or_envelope_opt を ArcSwap ベースに置換し、借用系 API を同期化。ベンチでは若干のばらつきが残るため、追加の最適化（割り当て削減など）を検討中。
- [ ] **ContextDecorator/Middleware 連鎖の同期化** - ContextDecoratorChain/ReceiverMiddlewareChain を ContextSnapshot (borrow + ArcSwap スナップショット) ベースへ再設計し、`ContextHandle::new` 多重生成を排除する。lock-metrics の read ロックをゼロに近づけるため、同期パス優先＋async フォールバックの二段構えを整備する。
- [ ] **Supervisor メトリクスの周辺プロセス展開** - DeadLetterProcess や ActorFutureProcess へ ArcSwap<MetricsRuntime> の同期アクセスを拡張し、メトリクス API を統一する。

### 完了項目
- ✅ `TypedActorHandle` / `PidActorRef` の弱参照化を完了 (2025-09-25)
- ✅ `ReceiverContextHandle` / `TypedContextHandle` に同期 borrow API を追加し、`ReceiverPart` ホットパスのロックを回避 (2025-09-25)
- ✅ SupervisorHandle の同期 borrow 化を完了（SupervisorCell を主記憶として RwLock を排除, 2025-09-26）
- ✅ リグレッション用のスプリント別タスク整理と完了チェック運用の自動化（scripts/report_todos.py 追加, 2025-09-26）
- ✅ `async_trait` 依存を棚卸しし、同期化できるメソッドを通常メソッドへ置換（2025-09-26 時点、追加置換対象なし）
- ✅ `TypedContext` 系 getter の同期化可能箇所を再調査し、代替 API を策定・実装（同期 getter 再設計メモ参照, 2025-09-26）
- ✅ protoactor-go (`actor/context.go`, `actor/supervision.go`) の呼び出し順序差分を `docs/dispatcher_runtime_policy.md` へ反映するドラフトを作成（2025-09-26）
- ✅ TypedActorContext 系の同期化済み borrow に合わせて情報取得メソッドを再整理する（2025-09-26）
- ✅ ReceiverMiddlewareChain 実行部で `context_cell_stats()` の差分をログ収集する（ヒット/ミスをデバッグ出力, 2025-09-26）
- ✅ `SupervisorHandle::new(self.clone())` など既存生成箇所に `inject_snapshot()` を組み込み、初期化直後からスナップショットが利用されるか確認する（2025-09-26）

### 同期 getter 再設計メモ（2025-09-26 更新）
- 実装進捗: `ActorContext::try_sender` / `ContextHandle::try_get_sender_opt` を導入し、TypedContextHandle/TypedActorContext/TypedRootContext に `sync_view()` を実装。Remote/Cluster のホットパスでは `try_get_message_handle_opt` と送信者スナップショットを計測用に組み込み済み。
- 計測: `remote/src/metrics.rs` と `cluster/src/metrics.rs` に同期 sender 取得のヒット/ミスを収集するベンチ用ユーティリティを追加。`nexus-actor-remote-rs` では既存テストから同期取得のミス計測を検証。
- 目的: `TypedContext` 系のホットパスから await を除去し、`ContextBorrow<'_>` を中心としたライフタイム指向 API と整合させる。
- 同期化しやすい getter: `get_parent` / `get_self_opt` / `get_actor` / `get_actor_system` / メッセージ系は `ContextHandle::actor_context_arc()` 由来のスナップショットからクローンを返せる。デコレーター経路や RootContext などで `None` が返る場合は Option で扱う。
- 同期化が難しい箇所: 送信者取得は同期アクセサが無いため `ActorContext::try_sender()` と `ContextHandle::try_get_sender_opt()` 追加が前提。`M: Clone` 制約や `RwLock::try_read` 失敗時の `None` 返却を仕様として文書化する。
- 2025-09-26 計測: `cargo bench -p nexus-actor-bench --features lock-metrics` の `context_borrow/borrow_hot_path` で read lock 654,000 回 (約 2,000/iteration), write lock 0, snapshot hit 1,308,654, snapshot miss 0。`ctx.get_message_handle_opt().await` 以外のルート (RootContext/request_future経路など) で read lock が維持されている可能性があるため、対象 API の分解と同期アクセサ適用範囲の再確認が必要。
- 設計方針: `TypedContextSyncView` を実装体付きに拡張し、`TypedContext` へ `sync_view()`（仮称）を追加。内部で `ContextBorrow` と `ContextCell` のスナップショットを束ねた `TypedContextSnapshot` を返し、取得できなかった項目は既存 async getter へフォールバックする。
- 残タスク: ContextHandle に送信者スナップショット API を追加、TypedContextHandle / TypedActorContext / TypedRootContext に `TypedContextSyncView` 実装を提供、`ContextAdapter` やベンチ用コードを `sync_view()` に移行、関連ドキュメント（`docs/typed_context_guidelines.md` など）を更新。
- 2025-09-26 計測: `cargo bench -p nexus-actor-bench --features lock-metrics` の `context_borrow/borrow_hot_path` で read lock 654,000 回 (約 2,000/iteration), write lock 0, snapshot hit 1,308,654, snapshot miss 0。`ctx.get_message_handle_opt().await` 以外のルート (RootContext/request_future 経路など) で read lock が維持されている可能性があるため、対象 API の分解と同期アクセサ適用範囲の再確認が必要。
- 2025-09-26 追試: `cargo bench -p nexus-actor-bench --bench reentrancy` で `context_borrow/borrow_hot_path` は 32.08ms (変化域 −1.5%〜+0.27%)。`--features lock-metrics` 付きでは 32.28ms（約 −8.8% 改善）と揮発する結果。ArcSwap 化による追加 `Arc` 割り当てがオーバーヘッドとなる可能性があるため、引き続きプロファイラでの解析とメモリ割り当ての削減策（専用セル構造検討など）が必要。
- 次ステップ: メッセージセルの追加割り当て削減 PoC を SmallVec / 再利用 Arc / 専用セルの3案で実施し、`context_borrow/borrow_hot_path` の alloc 指標を比較する。採用案を pipeline と ContextHandle に反映し、ドキュメントへ記録する。
- Decorator/Middleware 連鎖は ContextSnapshot (borrow + sender/receiver スナップショット) を入力とする同期 API へ置換し、async フォールバックのみ Future を生成する構成を検討する。`ContextHandle::new` 多重生成箇所の棚卸しと lock-metrics の read ロック再測定を行う。
- `cargo make lifetime-regression` で tests + coverage を一括実行するタスクを追加。CI/手動回帰時はこのタスクを利用する。

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
- ✅ `TypedActorHandle` / `PidActorRef` の弱参照化を完了 (2025-09-25)

### フェーズ C: 観測と回帰防止
**完了済み**
- ✅ `scripts/check_context_borrow_bench.sh` を CI へ統合
- ✅ `scripts/report_todos.py` を追加し、docs/core_improvemnet_plan.md の未完了タスクを自動一覧化（2025-09-26）

**残タスク（TODOサマリ参照）**

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
  - ReceiverContextHandle / TypedContextHandle: `try_*` 系の同期メッセージアクセスと `with_actor_borrow` クロージャ API を追加し、`ReceiverPart` ホットパスで `await` とロック取得を回避できるようにした。
  - SupervisorHandle: SupervisorCell を直接主記憶として利用し、`Arc<RwLock>` 依存を廃止。同期的な `supervisor_arc()` / `get_supervisor()` から `Arc<dyn Supervisor>` を取得できるようになった。

#### ContextHandle / Supervisor API 差分（2025-09-25 調査）
- `ContextHandle` は `Arc<RwLock<dyn Context>>` を保持しつつ、`ContextCell` 経由で `ActorContext` のスナップショットを `ArcSwapOption` で公開するよう改修（`core/src/actor/context/context_handle.rs:24-212`）。protoactor-go と同様に借用中のロック保持を避けるには、`ContextCell` の read/write 双方を活用する追加改修が必要。
- `ReceiverContextHandle` / `TypedContextHandle` は `try_*` 系同期 API を備え、`ContextCell` スナップショットと `ContextBorrow` を活用したホットパス参照が可能になった。
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
- ✅ `ReceiverContextHandle` / `TypedContextHandle` に同期 borrow API を提供し、`ReceiverPart` などの高速パスでロックを取らないようにする（`try_*` 系 API と `with_actor_borrow` を実装, 2025-09-25, 対象: `core/src/actor/context/receiver_context_handle.rs`, `core/src/actor/context/typed_context_handle.rs`）
- ✅ SupervisorHandle から `Arc<RwLock>` を排除し、スナップショット経由の同期参照に一本化（2025-09-26, `core/src/actor/supervisor/supervisor_strategy.rs`）
- ✅ deprecated 属性を既存 async getter 等へ付与し、`try_*` 系同期 API への移行を促進 (2025-09-26)

**残タスク（TODOサマリ参照）**

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
- **完了メモ**
- ✅ TypedActorContext 系の情報取得メソッドを `borrow()` + `try_*` 優先に再整理し、フォールバックとしてのみ async getter を使用する構成へ更新（2025-09-26）。

**完了済み / 進捗メモ**
- ✅ ContextHandle 系の `Arc<Mutex>` はメッセージセル制御に依存しており、`ArcSwap` 化 PoC を実装しつつ `RwLock` 維持で順序保証を確認。
- ContextBorrow 内のメトリクス経路は `ArcSwapOption<MetricsRuntime>` を参照する PoC に更新済み。`metrics_sink()` 経由で await を挟まない取得が可能になったため、Supervisor 側の呼び出し線図を同期化した後に最終的な API 置換を実施する予定。
- ✅ `ActorHandle::type_name`: 型名取得のための `await` を `Arc<str>` キャッシュ化で同期メソッドに変更（2025-09-25）
- ✅ `ActorContext::get_actor` / `set_actor`: `ArcSwapOption` 化により同期アクセサへ置換済み（2025-09-25）
- ✅ `ActorContext::borrow`: メトリクス参照を除き同期化完了。`self_pid` を `RwLock` から `ArcSwapOption` へ移行する案を検討中。
- ✅ `Task::run` を同期メソッドへ変更し、テストユーティリティの `async_trait` 依存を削減 (2025-09-26)
- `TypedActorContext::borrow`: 同期版 `ActorContext::borrow` に合わせて単なるラッパーへ変更予定。
- `ContextHandle` 系 `get_message_handle`: 既存ロック再利用のため非同期維持。差し替え難度が高いため保留。

## 計測と検証
- テスト: `cargo test --workspace`
- ベンチ: `cargo bench -p nexus-actor-bench --bench reentrancy`
- 閾値: CI では `REENTRANCY_THRESHOLD_NS` と `CONTEXT_BORROW_THRESHOLD_MS` をリポジトリ変数で管理。
- ダッシュボード/CSV 化はバックログ: 実装タスク完了を優先し、`docs/bench_dashboard_plan.md` のワークフロー整備は後続スプリントで再開予定。


## リスクと対策
- **性能退行**: ライフタイム化に伴う同期手段の変更でベンチ数値が悪化する可能性。→ 変更毎に `scripts/check_*_bench.sh` を実行し、CI のアラートを必ず確認。
- **API 破壊**: `ContextBorrow` 導入に伴い旧 API の互換性が失われる恐れ。→ 破壊的変更は README / CHANGELOG へ明示し、protoactor-go 読み替え指針を添付。
- **テスト不足**: ライフタイム関連の退行は静的解析で検知しづらい。→ `cargo miri` や `loom` を用いた追加検証を検討。

## 参考ドキュメント
- `docs/typed_context_guidelines.md`
- `docs/dispatcher_runtime_policy.md`
- `docs/bench_dashboard_plan.md`
- protoactor-go: `@docs/sources/protoactor-go`（Go 実装をライフタイム指向に読み替える際の参照）
