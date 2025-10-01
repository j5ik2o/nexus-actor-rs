# ActorContextExtras 同期化メモ (2025-09-29 時点)

## 区分基準
- **現状構成**: 実装済みのロック構造と API。
- **改善余地**: 追加で検討している変更点。
- **リスク**: 同期化に伴う懸念事項。

## 現状構成
- `ActorContextExtras` は `InstrumentedRwLock<ActorContextExtrasMutable>` を保持し、可変領域（receive timeout timer / restart stats / stash）のみ `RwLock` 経由で操作（`modules/actor-core/src/actor/context/actor_context_extras.rs`）。
- `children` / `watchers` は同期化した `PidSet` を保持し、`get_children` / `get_watchers` が即時クローンを返す。
- `context` 参照は `ArcSwapOption<WeakContextHandle>` で管理され、Borrow/Snapshot API から同期的に取得可能。
- ReceiveTimeoutTimer は `RwLock` 包みの DelayQueue を維持するが、初期化／リセットはロックを最小化するよう分割済み。

## 改善余地
- `ActorContextExtrasMutable` の `OnceCell<RestartStatistics>` は `get_or_init` 時に write ロックが必要なため、統計利用頻度に応じて別ロックへ切り出す案を継続検討。
- `ContextExtensions` との連携で重複ロックが発生しないか、tokio-console で継続監視。
- DelayQueue 置換案（`docs/benchmarks/receive_timeout_delayqueue.md`）が固まったら、Timer 実装を差し替える。

## リスク
- `InstrumentedRwLock` のメトリクス出力頻度が高いとホットパスでオーバーヘッドが増える可能性。サンプリング間隔の調整を行う。
- `PidSet` クローンは `Vec` のコピーを伴うため、巨大な子アクター集合を扱う際は `for_each` API への移行を推奨。

