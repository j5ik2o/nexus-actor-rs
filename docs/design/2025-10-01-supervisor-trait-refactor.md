# Supervisor 抽象再設計プラン (2025-10-01)

## 区分基準
- **現状課題**: Supervisor 周りに残る Tokio/std 依存のうち、core へ抽象化すべき要素。
- **抽象設計**: core 側で新設するトレイト／型の仕様と責務分割。
- **アダプタ方針**: actor-std 側で既存実装をラップするための橋渡し構成。
- **移行ステップ**: 実装・検証・ドキュメント更新の段階整理。

## 現状課題
- SupervisorStrategy と Supervisor トレイトが `async_trait` ベースで Tokio ランタイム／`ActorSystem` に直接依存しており、no_std 層から利用できない。
- SupervisorDirective (`Directive`) や RestartStatistics が std 側に偏在し、core からはメッセージ再送や停止を直接操作できない。
- メトリクス記録 (`record_supervisor_metrics`) や Decider 実装が std レイヤのみに存在するため、代替ランタイム向けの Supervisor 実装が困難。

## 抽象設計
1. **CoreSupervisorDirective**
   - actor-core へ `enum CoreSupervisorDirective { Resume, Restart, Stop, Escalate }` を追加。
   - `Directive` は std 側で `From<CoreSupervisorDirective>` を実装して再利用。
2. **コアトレイト群**
   - `CoreSupervisorFuture<'a, T>` / `CoreSupervisorStrategyFuture<'a>` を `Pin<Box<dyn Future<Output = T> + Send + 'a>>` で定義（Mailbox のパターンと揃える）。
   - `trait CoreSupervisorStrategy`:
     - `fn handle_child_failure(&self, ctx: &mut dyn CoreSupervisorContext, reason: ErrorReasonCore, message: MessageHandle)` → Future 返却スタイル。
     - `fn as_any(&self) -> &dyn Any` でダウンキャストを維持。
   - `trait CoreSupervisor`:
     - `fn children(&self) -> CoreSupervisorFuture<'_, Vec<CorePid>>`
     - `fn apply_directive(&self, directive: CoreSupervisorDirective, targets: &[CorePid], reason: ErrorReasonCore, message: MessageHandle)`
     - `fn escalate(&self, reason: ErrorReasonCore, message: MessageHandle)`
   - `struct CoreSupervisorContext` を導入し、`ActorSystem` 依存を排除して必要な情報（`metrics_sink`, `deadline`, `clock` など）を集約。
3. **ErrorReason/RestartStatistics**
   - 既に `RestartStatistics` は core に存在（`modules/actor-core/src/actor/core_types/restart.rs`）。`ErrorReason` も core へ再配置し、std 側は拡張ヘルパーのみ保持。

## アダプタ方針 (actor-std)
1. **Strategy Adapter**
   - `struct StdSupervisorStrategyAdapter<S>` で `CoreSupervisorStrategy` を実装。内部で既存 `SupervisorStrategy` を保持し、`handle_child_failure` 内で tokio ランタイムへ委譲。
2. **Supervisor Adapter**
   - `struct StdSupervisorAdapter` が `CoreSupervisor` を実装。
   - 既存 `SupervisorHandle` から `Arc<dyn Supervisor>` を取得し、Core トレイト呼び出しで `Directive` へのマッピングを行う。
3. **Context 拡張**
   - `CoreSupervisorContext` から std 依存のメトリクスや ActorSystem を参照するための trait object (`StdSupervisorContextExtensions`) を導入。
4. **Decider・Metrics**
   - core 側には最小限の Decider トレイト (`fn decide(&self, reason: &ErrorReasonCore) -> CoreSupervisorDirective`) を定義。
   - std 側は既存 `Decider` を adapter 経由で wrap。

## 移行ステップ
1. **Core 側基盤追加**
   - `modules/actor-core/src/supervisor/mod.rs` を新設し、directive / context / trait / future type alias を実装。
   - `actor::core_types::pid` など既存型を再利用しながら ErrorReasonCore を core に移設。
2. **Std 側実装調整**
   - 新規アダプタ (`core_queue_adapters` と同様の構成) を `modules/actor-std/src/actor/supervisor` 下に追加。
   - 既存 Strategy（one-for-one 等）を Core トレイト実装に変更しつつ、Tokio 固有部分は adapter 経由に限定。
3. **メトリクス/イベント橋渡し**
   - `record_supervisor_metrics` 等は adapter でのみ呼び出し、core トレイト経由では no-op（hook）可能にする。
   - `SupervisorEvent` 等の通知は core 側で trait を定義し、std adapter が EventStream を発火。
4. **テスト整備**
   - コア層：`cargo test -p nexus-actor-core-rs --no-default-features --features alloc` で新トレイトのユニットテストを追加。
   - Std 層：既存 supervisor テストを adapter 経由に書き換え、Tokio 依存が std 側に閉じていることを確認。
5. **ドキュメント更新**
   - `docs/worknotes/2025-10-01-actor-split-plan.md` に進捗を追記。
   - リリースノート草案 (`docs/releases/2025-10-01-actor-core-std-split.md`) で supervisor 抽象移行を記載。

## リスクと緩和策
- **非同期パフォーマンス**: adapter 層での `BoxFuture` 化に伴うオーバーヘッド → `#[inline]` 指定と `Pin<Box<...>>` 再利用で最小化。
- **API 互換**: std 側の公開 API が変わらないよう、`SupervisorStrategyHandle` など既存ハンドルは引き続き同じメソッドを提供する。
- **段階導入**: まず core トレイト・directive のみ追加し、旧コードを同時に保持するブリッジ期間を設ける。テストが安定した段階で旧トレイトを削除する。

