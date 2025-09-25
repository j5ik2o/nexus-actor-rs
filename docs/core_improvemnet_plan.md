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

## Sprint 3 次のアクション
1. `Arc<Mutex<_>>` 使用箇所の棚卸し (`rg "Arc<Mutex" core/`) を自動化し、ライフタイム移行対象リストを `docs/typed_context_guidelines.md` に追記。
2. ~~`ActorContextShared` の `ArcSwapOption` 版に合わせ、`ContextBorrow` からのアクター参照 API を整理（不要な `await` を削減し、再起動時の整合性を確認）。~~ ✅ `ActorContext::borrow` を同期化済み（2025-09-25）
3. Extension レイヤの borrow API を設計し、`ContextExtensionHandle` 経由の clone を削減する。（`RwLock` 版での読み取りアクセサ設計を含む）
4. 本計画書の更新作業を行った場合は、必ず進捗（ロードマップのチェックボックスや次アクション）を見直し、最新状態へ反映する。

## Sprint 4 次のアクション
1. Supervisors/metrics 経路の async 呼び出しを棚卸しし、同期コンテキストと共存させる設計案をまとめる。
2. ContextHandle の `Arc<Mutex>` 利用箇所を `ArcSwap` 化する PoC を検討し、受信順序の保証方法を調査。
3. Extension レイヤの borrow API を設計し、`ContextExtensionHandle` の clone を削減する。
4. 本計画書の更新作業を行った場合は、必ず進捗（ロードマップのチェックボックスや次アクション）を見直し、最新状態へ反映する。

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
- ダッシュボード: `docs/bench_dashboard_plan.md` の通り週次アーティファクトを `benchmarks/history` に集約予定。

## リスクと対策
- **性能退行**: ライフタイム化に伴う同期手段の変更でベンチ数値が悪化する可能性がある。→ 変更毎に `scripts/check_*_bench.sh` を実行し、CI のアラートを必ず確認する。
- **API 破壊**: `ContextBorrow` 導入に伴い旧 API の互換性が失われる恐れ。→ 破壊的変更は README / CHANGELOG へ明示し、protoactor-go 読み替え指針を添える。
- **テスト不足**: ライフタイム関連の退行は静的解析で検知しづらい。→ `cargo miri` や `loom` を用いた追加検証を検討。

## 参考ドキュメント
- `docs/typed_context_guidelines.md`
- `docs/dispatcher_runtime_policy.md`
- `docs/bench_dashboard_plan.md`
- protoactor-go: `@docs/sources/protoactor-go`（Go 実装をライフタイム指向に読み替える際の参照）
