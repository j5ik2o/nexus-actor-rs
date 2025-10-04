# Dispatcher Runtime ポリシー (2025-09-29 時点)

## 区分基準
- **既存ガイドライン**: 現行 Dispatcher 実装で遵守しているルール。
- **適用時の注意点**: 新規 Dispatcher を追加する際の留意事項。

## 既存ガイドライン
- `modules/actor-core/src/actor/dispatch/dispatcher/single_worker_dispatcher.rs` は `Drop` で `Runtime::shutdown_background()` を呼び、Tokio ランタイムの二重解放を防止。
- ランタイム所有は `Option<Handle>` ではなく `Option<Arc<Runtime>>` で保持し、`ActorSystem` 側と参照カウントを共有する実装に統一。
- `CurrentThreadDispatcher` のように外部ランタイムを利用する実装はランタイムを保持せず、上記パターンに該当しない。

## 適用時の注意点
- 新規 Dispatcher で独自ランタイムを生成する場合は、`Drop` 実装に `shutdown_background` を必ず追加する。
- ランタイムを保持しない Dispatcher には明確に `RuntimeOwnership::Borrowed` のような enum ラッパーを用意し、API 差異を明らかにする（別タスクで検討）。

