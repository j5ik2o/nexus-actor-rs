# Typed Context / PID ガイドライン

## 目的
- `ContextBorrow<'_>` を用いたライフタイム安全な参照取得の流れを整理する。
- `ContextHandle::try_into_actor_context` / `ActorContext::borrow` を利用する際の注意点を共有する。
- `WeakActorSystem` への移行による循環参照防止ポリシーを明記する。

## 推奨パターン
1. **TypedActorContext<M> を活用**
   - `TypedActorContext<M>` からは通常の `TypedInfoPart` / `TypedSenderPart` API を優先して利用する。
   - `ctx.get_message_handle_opt().await` で取得したメッセージは `to_typed::<M>()` で検証する。
   - より詳細な参照が必要な場合は `ctx.borrow()` を利用して `ContextBorrow<'_>` を取得し、props や self PID にアクセスする。

2. **`ContextHandle` からの `ActorContext` 取得**
   - 直接 `ContextHandle` を保持している場合は `ctx.try_into_actor_context().await` を利用する。
   - `None` が返るケース（デコレーターでラップされたコンテキストなど）に備え、`Option` を考慮したハンドリングを行う。

3. **`ContextBorrow<'_>` の使用**
   - `ActorContext::borrow()` で取得した `ContextBorrow<'_>` からは `props()` や `self_pid()` を参照できる。
   - 借用は短時間に留め、必要なら値をコピーしてから非同期処理を続行する。

4. **`WeakActorSystem` の扱い**
   - `ContextBorrow<'_>` から得られる `actor_system()` は強参照を返すが、内部では `WeakActorSystem` が利用されている。
   - 長時間保持する場合はガーベジ化の可能性に注意し、極力そのスコープ内でのみ使用する。

## よくあるアンチパターン
- 旧 API `snapshot()` / `get_props()` をコピー目的で乱用すること。
- `ContextHandle` から `to_actor_context()` を呼ぶことを前提にしたモジュール境界越えの依存。
- `ActorSystem` を構造体に強参照として保持し、循環参照を招く設計。

## 移行チェックリスト
- [ ] `ContextHandle` 経由の `ActorContext` 取得箇所が `try_into_actor_context` を利用しているか。
- [ ] `ActorContext` を保持する構造体が `WeakActorSystem` を使っているか。
- [ ] `TypedExtendedPid` / `TypedActorContext` を利用するコードで冗長な clone / Arc 共有をしていないか。

## `Arc<Mutex<_>>` 棚卸しと優先順位
- `scripts/list_arc_mutex_usage.sh` を実行することで、リポジトリ全体の `Arc<Mutex<_>>` 使用箇所を一覧化できる。
- 2025-09-25 時点で高優先度と判断した箇所:
  - ~~`core/src/ctxext/extensions.rs` : Extension 管理ベクタを `Arc<Mutex<Vec<Option<ContextExtensionHandle>>>>` で保持。読み取り主体のため `RwLock` + 借用に移行予定。~~ ✅ `borrow_extension` / `borrow_extension_mut` API で参照取得可
  - `core/src/metrics/actor_metrics.rs` : メトリクス更新でロック粒度が大きく、`ContextBorrow` 経由で必要データを渡す設計へ移行検討。
  - `core/src/actor/supervisor/supervisor_strategy.rs` : `SupervisorHandle` を `Arc<RwLock<dyn Supervisor>>` 化し、再入ロックを削減済み。今後は `ContextBorrow` ベースの API 拡張を検討する。
- `remote/src/endpoint_manager.rs` / `remote/src/remote.rs` : リモート起動経路で `Arc<Mutex<Option<_>>>` が多用され、タイムアウト処理と競合。ライフタイム移行後は `OnceCell` + 借用で行ロックを排除する計画。

棚卸し結果は `docs/core_improvemnet_plan.md` のロードマップと同期し、移行作業の進捗に合わせて定期的に更新すること。

## 参考
- `core/src/actor/context/actor_context.rs` `ContextBorrow` 実装
- `core/src/actor/core_types/adapters.rs` `ContextAdapter`
- `bench/benches/reentrancy.rs` `BorrowingActor` のサンプル
