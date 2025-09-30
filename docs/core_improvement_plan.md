# core ライフタイム移行メモ (2025-09-29 時点)

## 区分基準
- **同期化の現状**: 既に main ブランチへ反映された変更点。
- **検証タスク**: 今後の評価・改善でフォローすべき項目。

## 同期化の現状
- `modules/actor/src/actor/context/actor_context.rs` と `context_handle.rs` は `ContextBorrow` / `ContextSnapshot` を提供し、ミドルウェアが同期参照で完結するパスを確保済み。
- Supervisor・メトリクス周りは `ArcSwap` に統一され、`ContextExtensions` も `ArcSwapOption<WeakContextHandle>` ベースで共有（`modules/actor/src/actor/context/actor_context_extras.rs`）。
- `scripts/list_arc_mutex_usage.sh` で棚卸しした `Arc<Mutex<_>>` の主要箇所は `PidSet`・`ActorContextExtras` へ移行し、同期ロックによる再入リスクを低減済み。

## 検証タスク
- Virtual Actor 経路（`cluster/src/virtual_actor/runtime.rs`）で `ContextHandle` の同期 API を多用する箇所を洗い出し、`ContextBorrow` へ置換できるか確認する。
- `loom` ベースの並行検証を導入し、`InstrumentedRwLock` をまたいだデッドロック検知を自動化するか検討する。
- `ContextExtensions` 経由で `ContextHandle::with_typed_borrow` を呼ぶホットパスのベンチ（`modules/bench/benches/reentrancy.rs`）を継続監視し、競合が再発しないか計測する。
