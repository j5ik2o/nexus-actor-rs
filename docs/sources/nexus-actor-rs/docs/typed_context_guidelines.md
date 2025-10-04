# Typed Context / PID ガイドライン (2025-09-29 時点)

## 区分基準
- **目的**: ガイドライン全体の意義。
- **推奨パターン**: 実装で遵守すべき具体的手順。
- **アンチパターン/チェックリスト**: 避けるべき実装と未完タスクの確認。

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

5. **コンテキスト/ミドルウェアの再利用**
   - `ActorContext` は `context_handle()` メソッドを公開し、`OnceCell` キャッシュ経由で `ContextHandle` を再利用する。Props.on_init など所有 ActorContext を持つ場面では、この同期ハンドルを取得して再利用すること。
   - 既に `ActorContext` インスタンスがある場合は `context_handle()` を使用する。`ContextHandle::new(self.clone())` の直接呼び出しは避け、既存のハンドルか `ContextSnapshot` / `ContextHandle::snapshot()` を活用してスナップショットを引き渡す。

   **TypedContextSnapshot の活用**
   - `TypedContextSnapshot` は型付きコンテキストの不変スナップショットを提供する。このスナップショットは作成時点の状態を保持し、元のコンテキストでメッセージや sender がクリアされても既存のスナップショットの内容は維持される。
   - `TypedActorContext` と `TypedRootContext` は `snapshot()` メソッドを提供し、現在の状態の `TypedContextSnapshot` を返す。
   - スナップショットは `Debug` トレイトを実装しており、デバッグやロギング時に有用。

   **TypedContextBorrow の利用**
   - ライフタイム付きの参照だけで十分な場合は `ActorContext::with_typed_borrow::<M, _, _>` または `ContextHandle::with_typed_borrow::<M, _, _>` を利用し、`TypedContextBorrow<'_>` から props/self/sender を同期参照する。
   - `TypedContextBorrow` はメッセージや sender を同期的に取得でき、必要になったタイミングだけ `TypedContextBorrow::snapshot()` で所有スナップショットを生成できる。
   - Decorator/Middleware などホットパスではまず借用ビューで処理し、スナップショット生成はフォールバックに留めることで割り当てとロックを抑制できる。

   **ContextHandle Snapshot の活用**
   - `ContextHandle::snapshot()` は不変スナップショットを提供し、`ContextAdapter` などのアダプターレイヤーなしで状態を参照できる。
   - 同期参照を優先する場合は `try_get_message_handle_opt()` や `try_get_sender_opt()` を利用し、必要なときのみスナップショット化することでロック競合を最小化できる。
   - 旧アダプターレイヤー（ContextAdapter）は撤去されたため、直接 `ContextHandle` / `TypedContextHandle` API を利用すること。

   - Receiver ミドルウェアを実装する際は `ReceiverMiddleware::from_sync` / `from_async` を用い、`ReceiverSnapshot` を入力に同期パス優先で処理する。必要に応じて非同期フォールバックへ委譲することで lock-metrics 上の read ロックを最小化できる。

## よくあるアンチパターン
- 旧 API `snapshot()` / `get_props()` をコピー目的で乱用すること。
- `ContextHandle` から `to_actor_context()` を呼ぶことを前提にしたモジュール境界越えの依存。
- `ActorSystem` を構造体に強参照として保持し、循環参照を招く設計。

## 移行チェックリスト
- [ ] `ContextHandle` 経由の `ActorContext` 取得が `try_into_actor_context` ベースに統一されているか（例: `modules/actor-core/src/actor/core/child/tests.rs` では `to_actor_context` 依存が残存）。
- [x] `ActorContext` を保持する構造体が `WeakActorSystem` を使っているか（`modules/actor-core/src/actor/context/actor_context.rs`・`root_context.rs` で確認済み）。
- [ ] `TypedExtendedPid` / `TypedActorContext` を利用するコードで冗長な clone / Arc 共有をしていないか（`modules/actor-core/src/actor/context/typed_actor_context.rs` の send/request 系で再確認が必要）。

## `Arc<Mutex<_>>` 棚卸しと優先順位
- `scripts/list_arc_mutex_usage.sh` を実行することで、リポジトリ全体の `Arc<Mutex<_>>` 使用箇所を一覧化できる。
- 2025-09-25 時点で高優先度と判断した箇所:
  - ~~`modules/actor-core/src/ctxext/extensions.rs` : Extension 管理ベクタを `Arc<Mutex<Vec<Option<ContextExtensionHandle>>>>` で保持。読み取り主体のため `RwLock` + 借用に移行予定。~~ ✅ `borrow_extension` / `borrow_extension_mut` API で参照取得可
  - `modules/actor-core/src/metrics/actor_metrics.rs` : メトリクス更新でロック粒度が大きく、`ContextBorrow` 経由で必要データを渡す設計へ移行検討。
  - `modules/actor-core/src/actor/supervisor/supervisor_strategy.rs` : `SupervisorHandle` を `Arc<RwLock<dyn Supervisor>>` 化し、再入ロックを削減済み。今後は `ContextBorrow` ベースの API 拡張を検討する。
- `remote/src/endpoint_manager.rs` / `remote/src/remote.rs` : リモート起動経路で `Arc<Mutex<Option<_>>>` が多用され、タイムアウト処理と競合。ライフタイム移行後は `OnceCell` + 借用で行ロックを排除する計画。

棚卸し結果は `docs/core_improvement_plan.md` のロードマップと同期し、移行作業の進捗に合わせて定期的に更新すること。

## 参考
- `modules/actor-core/src/actor/context/actor_context.rs` `ContextBorrow` 実装
- `modules/actor-core/src/actor/context/context_handle.rs` `ContextHandle` スナップショット API
- `modules/actor-core/benches/reentrancy.rs` `BorrowingActor` のサンプル
