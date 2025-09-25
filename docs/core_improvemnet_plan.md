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

## ロードマップ
### フェーズ A: ActorContext コアの再編
- [x] `ContextBorrow<'_>` による props/self 参照の提供
- [x] `WeakActorSystem` 化と循環参照の除去
- [x] `ActorContext` 内部ロックの分離 (`extras` / `message_or_envelope`)
- [x] 初期ベンチ (`context_borrow`) の追加
- [x] `ActorContextShared` を `OnceCell<ArcSwapOption<ActorHandle>>` ベースに刷新し、アクター差し替え時のロックを排除

### フェーズ B: コンテキスト拡張と Supervisor
- [x] Extension 管理 (`ctxext::extensions`) の `Arc<Mutex<Vec<_>>>` を `RwLock` + スライス借用に移行
- [x] Supervisor 階層の `Arc<Mutex<dyn Supervisor>>` を `Arc<RwLock<dyn Supervisor>>` に刷新し、再入ロックを削減
- [ ] `TypedActorHandle` / `PidActorRef` の弱参照化を完了

### フェーズ C: 観測と回帰防止
- [x] `scripts/check_context_borrow_bench.sh` を CI へ統合
- [ ] ベンチ結果を CSV 化して履歴追跡 (`scripts/export_bench_metrics.py`)
- [ ] `docs/bench_dashboard_plan.md` に沿った GitHub Pages 公開
- [ ] `cargo make coverage` をライフタイム回帰テストに組み込み
- [ ] リグレッション用のスプリント別タスク整理と完了チェック運用の自動化

## Sprint 3 次のアクション
1. ~~`Arc<Mutex<_>>` 使用箇所の棚卸し (`rg "Arc<Mutex" core/`) を自動化し、ライフタイム移行対象リストを `docs/typed_context_guidelines.md` に追記。~~ ✅ `scripts/list_arc_mutex_usage.sh` + ガイドライン更新済み
2. ~~`ActorContextShared` の `ArcSwapOption` 版に合わせ、`ContextBorrow` からのアクター参照 API を整理（不要な `await` を削減し、再起動時の整合性を確認）。~~ ✅ `ActorContext::borrow` を同期化済み（2025-09-25）
3. ~~Extension レイヤの borrow API を設計し、`ContextExtensionHandle` 経由の clone を削減する。（`RwLock` 版での読み取りアクセサ設計を含む）~~ ✅ `borrow_extension` / `borrow_extension_mut` を実装

## Sprint 4 次のアクション
1. Supervisors/metrics 経路の async 呼び出しを棚卸しし、同期コンテキストと共存させる設計案をまとめる。
2. ContextHandle の `Arc<Mutex>` 利用箇所を `ArcSwap` 化する PoC を検討し、受信順序の保証方法を調査。
3. ~~Extension レイヤの borrow API を設計し、`ContextExtensionHandle` の clone を削減する。~~ ✅ Sprint 3 で完了済み

### Sprint 4 詳細タスク
- **Supervisor/metrics 同期化設計案**: `core/actor/supervisor` と `core/metrics` の呼び出し線図を整理し、protoactor-go の `actor/context` 実装から同期 API の移植候補を抽出する。完了条件は、同期化可否を判断できるクラス図と、必要な API 変更草案を `docs/dispatcher_runtime_policy.md` に追記する PR 下書きが存在すること。
- **ContextHandle ArcSwap PoC**: `core/actor/context/context_handle.rs`（仮）に `ArcSwap<Option<MessageCell>>` ベースの PoC ブランチを作成し、`receive_ordering.rs` テストケースで message ordering を維持できることを示す。`protoactor-go/actor/context/context.go` の `ProcessMailbox` 相当のロジックを参照し、Rust 版の borrow パターンへ翻訳する。
- **実装優先のレビュー準備**: 上記 2 点の調査結果をもとに破壊的変更となる API 差分を洗い出し、Sprint 4 終了時に即実装へ着手できるよう `core/actor/context` 配下のタスクリストを整理する。

### Sprint 4 調査ログ
- Supervisors/metrics: `metrics_foreach` が OpenTelemetry extension 取得のため async を維持。同期化する場合は metrics extension を `ArcSwap` 化し、`ActorSystem::get_config()` も非同期依存を整理する必要がある。
- ContextHandle: メッセージセル (`message_or_envelope_opt`) の整合性確保のため `Arc<Mutex>` が必要。`ArcSwap` 化を行う場合はキュー投入順序と stashing を別構造へ退避する PoC を検討する。

#### ContextHandle / Supervisor API 差分（2025-09-25 調査）
- `ContextHandle` は `Arc<RwLock<dyn Context>>` を保持したままだが、新たに `ContextCell` を介して `ActorContext` のスナップショットを `ArcSwapOption` で公開するように改修（`core/src/actor/context/context_handle.rs:24-212`）。protoactor-go のように借用中のロック保持を避けるには、`ContextCell` を read/write 双方で活かす追加改修が必要。
- `ReceiverContextHandle` / `TypedContextHandle` も同様に `Arc<RwLock>` ベースで async を前提にしている（例: `core/src/actor/context/receiver_context_handle.rs:15-83`）。`ContextHandle` の差し替えと合わせて同期 API へ刷新する必要がある。
- `SupervisorHandle` は `Arc<RwLock<dyn Supervisor>>` に依存し、`borrow()` が async になっている（`core/src/actor/supervisor/supervisor_strategy.rs:71-165`）。protoactor-go の `supervision.go` は同期 API で `Supervisor` を操作しているため、`ArcSwap<dyn Supervisor>` + 即時 borrow を検討する。
- Supervisor イベントの購読は `subscribe_supervision` が `ActorSystem::get_event_stream()` を await している（`core/src/actor/supervisor/supervision_event.rs:18-34`）。同期化に合わせてイベント発火側も同期参照を想定した設計へ改修する必要がある。

##### 2025-09-25 実装メモ
- ReceiverContextHandle: `ContextHandle` を内包し、`actor_context_arc()` で `ContextCell` スナップショットを同期取得できる PoC を追加（`core/src/actor/context/receiver_context_handle.rs:16-92`）。
- TypedContextHandle: `actor_context_arc()` を公開し、Typed 経路でも同期スナップショットが参照可能に。
- SupervisorHandle: `SupervisorCell` を導入し、`ArcSwapOption<dyn Supervisor>` ベースのスナップショット API (`supervisor_arc`) を提供（`core/src/actor/supervisor/supervisor_strategy.rs:86-180`）。今後は RwLock の除去に向けてセル更新タイミングを整理する。
- 測定手段: `ContextCellStats` / `SupervisorCellStats` を追加し、`context_cell_stats()` / `supervisor_cell_stats()` からスナップショットのヒット数・ミス数を取得可能。ホットパス実行後に値差分を取ることで、同期参照化の効果と未カバー領域を定量化できる。
- `SupervisorHandle::inject_snapshot` を追加し、`SupervisorHandle::new_arc` など既存の RwLock ベース生成後でも任意タイミングでスナップショットを補足可能にした。`GuardianProcess` 等の構築時に Clone 済みインスタンスを渡せばセルを即座に同期化できる。

#### 実装タスクリスト（優先度順ドラフト）
1. `ContextHandle` を `ArcSwap` ベースへ刷新するための `ContextCell`（仮）を追加し、`Context` トレイト実装が同期的に借用できるよう `ContextBorrow<'_>` と統合する。（PoC: `ContextCell` 導入済み、同期 API 化は継続課題 / 対象: `core/src/actor/context/context_handle.rs`, `core/src/actor/context/actor_context.rs`）
2. `ReceiverContextHandle` / `TypedContextHandle` に同期 borrow API を提供し、`ContextHandle` 差し替え後も `ReceiverPart` などの高速パスでロックを取らないようにする。（PoC: `actor_context_arc()` を公開済み、呼び出し側の同期パス適用は継続課題 / 対象: `core/src/actor/context/receiver_context_handle.rs`, `core/src/actor/context/typed_context_handle.rs`）
3. `SupervisorHandle` を `ArcSwap<dyn Supervisor>` 化し、`SupervisorStrategy` 実装から同期 borrow が使えるよう `SupervisorCell` を導入する。必要なら `supervisor_strategy_handle.rs` の API も同期版に合わせて置換する。（対象: `core/src/actor/supervisor/supervisor_strategy.rs`, `core/src/actor/supervisor/supervisor_strategy_handle.rs`）
4. 上記に伴い発生する `async_trait` 依存の見直しを行い、同期化できるメソッドを `async` から通常メソッドへ変更する。変更後は `tokio::sync::RwLock` を削除し、`ContextBorrow` ベースのメトリクス経路が成立するかを確認する。
5. プロトタイプ段階では `protoactor-go/actor/context.go` と `protoactor-go/actor/supervision.go` の呼び出し順序を写経し、Rust 版での差分（所有権管理や lifetimes）を `docs/dispatcher_runtime_policy.md` に反映させる下書きを作る。

## Sprint 5 プランニング (案)
- **ArcSwap ベース ContextHandle の本実装**: Sprint 4 PoC のフィードバックを取り込み、`ContextHandle` / `ActorCell` 周辺の API を再設計する。protoactor-go の `SharedContext` / `ActorCell` に倣い、borrow 可能なメッセージセルを `Cow` でラップする案を検証。
- **Supervisor Metrics の段階的同期化**: `metrics_foreach` の非同期依存を `ArcSwap<MetricsRegistry>` 経由の同期アクセスへ移行し、OpenTelemetry exporter は別タスクへ切り出す。影響範囲を `docs/dispatcher_runtime_policy.md` と `docs/typed_context_guidelines.md` に反映。
- **回帰テスト拡充**: `cargo test -p core actor::context::tests::` に加え、`loom` を用いた並行試験のドラフトを準備。`protoactor-go/tests/context_test.go` を参照しつつ Rust 版の競合再現シナリオを作成する。

### Sprint 5 調査ログ（ドラフト）
- ArcSwap 化の副作用として、`ReceiverContext` でのメッセージ所有権移譲が複雑化する恐れがある。`ContextBorrow<'_>` の lifetime を `tokio::task::LocalSet` へ束縛する案を検証予定。
- Supervisor メトリクスの同期化は、`MetricsContext` での exporter 初期化順序がボトルネック。`WeakActorSystem` から exporter を取得する際の再入を避けるため、初期化専用タスクを追加する要否を評価する。
- ベンチダッシュボードや CSV 連携はバックログへ退避し、実装が安定した後に再検討する。現段階では `bench.yml` `bench-weekly.yml` の閾値監視のみを頼りにする。

## 運用ルール
- 本計画書を更新した際は、完了済み項目のチェックやスプリントの次アクションを必ず見直す。

### async -> sync 変換候補（メモ）
- TypedActorContext 系は borrow を同期化済み。情報取得メソッド群は underlying `ActorContext` の async を再利用しているため、Supervisor/metrics を同期化した後に再整理する。
- ContextHandle 系の `Arc<Mutex>` はメッセージセル制御に依存しており、`ArcSwap` 化は受信順序保証の検討が必要（棚卸し対象として継続）。

- ContextBorrow 内のメトリクス経路では `metrics_foreach` が async を要するため同期化は保留。メトリクス API 整理後に再検討。
- ~~`ActorContext::get_actor` / `set_actor` : `ArcSwapOption` 化済みのため同期アクセサへ置換予定。~~ ✅ 完了 (2025-09-25)
- ~~`ActorContext::borrow` : メトリクス参照を除き同期化可能。`self_pid` を `RwLock` から `ArcSwapOption` へ移行する案を検討。~~ ✅ 完了 (2025-09-25)
- `TypedActorContext::borrow` : 上記同期版 `ActorContext::borrow` に合わせて単なるラッパーへ変更。
- `ContextHandle` 系 `get_message_handle` : 既存ロック再利用のため非同期維持。差し替え難度が高いため保留。

## 計測と検証
- テスト: `cargo test --workspace`
- ベンチ: `cargo bench -p nexus-actor-bench --bench reentrancy`
- 閾値: CI では `REENTRANCY_THRESHOLD_NS` と `CONTEXT_BORROW_THRESHOLD_MS` をリポジトリ変数で管理。
- ダッシュボード/CSV 化はバックログ: 実装タスクの完了を最優先し、`docs/bench_dashboard_plan.md` のワークフロー整備は後続スプリントで再開する。それまでは既存スクリプトと CI 閾値によるチェックのみを運用する。

## リスクと対策
- **性能退行**: ライフタイム化に伴う同期手段の変更でベンチ数値が悪化する可能性がある。→ 変更毎に `scripts/check_*_bench.sh` を実行し、CI のアラートを必ず確認する。
- **API 破壊**: `ContextBorrow` 導入に伴い旧 API の互換性が失われる恐れ。→ 破壊的変更は README / CHANGELOG へ明示し、protoactor-go 読み替え指針を添える。
- **テスト不足**: ライフタイム関連の退行は静的解析で検知しづらい。→ `cargo miri` や `loom` を用いた追加検証を検討。

## 参考ドキュメント
- `docs/typed_context_guidelines.md`
- `docs/dispatcher_runtime_policy.md`
- `docs/bench_dashboard_plan.md`
- protoactor-go: `@docs/sources/protoactor-go`（Go 実装をライフタイム指向に読み替える際の参照）


1. ReceiverMiddlewareChain 実行部で context_cell_stats() の差分をログ出力し、実際のヒット率を収集する。
2. SupervisorHandle::new(self.clone()) など既存生成箇所に inject_snapshot() を組み込み、初期化直後からスナップショットが利用されるか確認する。