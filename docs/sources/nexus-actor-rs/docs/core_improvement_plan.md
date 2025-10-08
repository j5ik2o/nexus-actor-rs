# core ライフタイム移行メモ (2025-09-29 時点)

## 区分基準
- **同期化の現状**: 既に main ブランチへ反映された変更点。
- **検証タスク**: 今後の評価・改善でフォローすべき項目。

## 同期化の現状
- `modules/actor-core/src/actor/context/actor_context.rs` と `context_handle.rs` は `ContextBorrow` / `ContextSnapshot` を提供し、ミドルウェアが同期参照で完結するパスを確保済み。
- Supervisor・メトリクス周りは `ArcSwap` に統一され、`ContextExtensions` も `ArcSwapOption<WeakContextHandle>` ベースで共有（`modules/actor-core/src/actor/context/actor_context_extras.rs`）。
- `scripts/list_arc_mutex_usage.sh` で棚卸しした `Arc<Mutex<_>>` の主要箇所は `PidSet`・`ActorContextExtras` へ移行し、同期ロックによる再入リスクを低減済み。
- actor-core は `CorePid`／`CoreMessageEnvelope`／`CoreProcessHandle` を束ねた no_std + alloc の最小核として安定化し、actor-std は Tokio 実装と ProtoBuf 変換を担う実装ハブに一本化済み。

## 検証タスク
- Virtual Actor 経路（`cluster/src/virtual_actor/runtime.rs`）で `ContextHandle` の同期 API を多用する箇所を洗い出し、`ContextBorrow` へ置換できるか確認する。
- `loom` ベースの並行検証を導入し、`InstrumentedRwLock` をまたいだデッドロック検知を自動化するか検討する。
- `ContextExtensions` 経由で `ContextHandle::with_typed_borrow` を呼ぶホットパスのベンチ（`modules/actor-core/benches/reentrancy.rs`）を継続監視し、競合が再発しないか計測する。

## Embedded 対応計画（2025-10-03 着手）

### 区分基準
- **抽象一覧**: actor-core が要求するランタイム抽象を列挙し、no_std + alloc で維持すべき最小集合を定義。
- **Embassy 写像**: 上記抽象を Embassy 環境でどのプリミティブに対応づけるか整理。
- **追加課題**: 新規に設計・実装が必要となる領域を優先度付きで明示。

### 抽象一覧（MUST）
- `AsyncMutex<T>` / `AsyncRwLock<T>` / `AsyncNotify` / `AsyncYield` / `Timer` / `CoreScheduler` / `CoreRuntime(Config)` / `FailureClock` をコア抽象として維持し、`nexus_utils_core_rs::async_primitives` 由来のトレイトに集約する（`modules/utils-core/src/async_primitives.rs:1-138`）。
- それぞれの利用箇所を再確認し、Embassy 実装でガードが `Send` になるか、Future が `Pin<Box<_>>` で提供可能かをチェックする。

### Embassy 写像（SHOULD）
- `AsyncMutex` → `embassy_sync::mutex::Mutex`、`AsyncNotify` → `embassy_sync::signal::Signal`、`Timer` → `embassy_time::Timer::after` といった候補を採用し、`core::time::Duration` との変換ユーティリティを設計。
- `CoreScheduler` は `embassy_executor::Spawner` と `Timer` を組み合わせたタスク起動ラッパとして再構成し、once/repeated 双方を満たす API を定義する。
- `FailureClock` では `embassy_time::Instant` を利用する実装案をまとめ、再起動ポリシーが精度要件を満たすか評価する。

### 追加課題（MUST → SHOULD の順）
1. **RwLock 代替**: Embassy には RwLock が無いため、`AsyncRwLock` の内部実装を mutex + reader カウンタで代替するか、actor-core 側の読み書き要求を再設計する選択肢を比較する。
2. **Feature 設計**: `actor-embedded`（仮）クレートを追加し、`embassy-time`・`embassy-sync`・`embassy-executor` をオプション依存に設定。CI で `--no-default-features --features alloc,embedded` のビルドを検証する。
3. **ロギング方針**: actor-core にはロギング抽象が無いため、`tracing` / `defmt` などを注入するための Hook を定義し、std/embedded での実装方法をガイドとして整理する。

### 次アクション（2025-10 スプリント）
1. 抽象一覧と Embassy 写像の検証結果を `actor-embedded` 設計メモ（新規）へ落とし込み、RwLock 代替案の PoC 方針を決定する。
2. `actor-embedded` クレートの Cargo 雛形と feature 設計ドラフトを作成し、CoreRuntimeConfig に組み込むための API 変更案を提示する。
