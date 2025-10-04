# RestartStatistics 抽象化ドラフト (2025-10-01)

## 現状整理（分類軸: 依存要素 / API 特性）
- **依存要素**
  - `std::time::Instant` に依存しており `no_std` 環境では利用不可。
  - `nexus_utils_std_rs::concurrent::SynchronizedRw` を用いた Tokio ロック前提の実装。
  - フェイル履歴は `Vec<Instant>` を共有参照 (`Arc`) で保持しており、時間計測とメモリ確保が std 層に閉じている。
- **API 特性**
  - `fail` / `push` / `reset` / `number_of_failures` など全メソッドが `async fn` で、Tokio ランタイムに依存。
  - 再起動判定 (`should_stop`) は呼び出し側が `Duration` を経過時間に用いており、core 層側での判定ロジック再利用が難しい。
  - スーパーバイザ戦略からは `RestartStatistics` を値渡しで受け取り、コピー (Arc clone) が頻繁に発生。

## 抽象化方針案（分類軸: レイヤ責務 / データ構造）
- **レイヤ責務**
  1. `actor-core`: フェイル履歴を保持する `CoreRestartTracker`（仮称）トレイトと汎用データ構造を提供。
     - 失敗タイムスタンプは `u64`（monotonic tick）で保持し、`alloc` のみで動作。
     - 時刻取得は新設する `FailureClock` トレイトで外部注入（std 層は `Instant` をラップ、embedded は HAL 由来 tick を想定）。
     - API は同期メソッドで定義し、`async` 版は std 層が必要に応じてラップ。
  2. `actor-std`: `FailureClock` の Tokio 実装（`TokioInstantClock` 仮）と、既存 `RestartStatistics` を `CoreRestartTracker` のアダプタに置換。
     - Tokio ロック (`SynchronizedRw`) を維持する場合は `CoreRestartTracker` を内部に抱え、非同期 API を再導出。
- **データ構造**
  - `CoreRestartWindow`（仮）: `Vec<u64>` を保持し、`max_len` や `within_duration` の判定ロジックを提供。
  - `FailureSample` 型を導入し、`request_id` 等メタデータ追加にも対応しやすくする。

## マイグレーション手順案（段階: ①下準備 → ②移植 → ③置換）
1. **下準備**
   - `actor-core` に `failure_clock.rs`（`FailureClock` / `SystemTick` 型）と `restart_tracker.rs`（`CoreRestartTracker` トレイト、`CoreRestartWindow` 実装）を追加。
   - `actor-std` へ Tokio 実装を追加 (`TokioFailureClock` + 既存構造体のラッパー)。
2. **移植**
   - スーパーバイザ戦略類 (`strategy_*`) を `CoreRestartTracker` トレイト越しに操作するようリファクタ。
   - `ActorContextExtras` の `RestartStatistics` 保持をコア抽象に差し替え、既存 API をアダプタ化。
3. **置換・クリーンアップ**
   - 旧 `RestartStatistics` の `Instant` 依存機能を段階的に撤去し、`actor-core` へメイン実装を移動。
   - テストを `CoreRestartWindow` ベースに再構成し、std 側は最小限の統合テストのみに縮小。

## 懸念事項（分類軸: 技術リスク / テスト戦略）
- **技術リスク**
  - `std::time::Instant` と直接比較した挙動の差異（ラップアラウンドや粒度）をどう扱うか。
  - `async fn` → 同期 API への変更によるロック粒度の再検討が必要（`PidSet` 同様に snapshot 取得コスト増が懸念）。
- **テスト戦略**
  - コア層ではダミー `FailureClock`（固定 tick／モック）を用いた determinisitc テストを作成。
  - std 層では既存 async テストを `TokioFailureClock` + `CoreRestartWindow` 組み合わせで回帰確認。
 - ベンチマーク (`strategy_one_for_one` 等) を clock 差し替え後も継続し、パフォーマンス退行を検知。

## 進捗メモ（2025-10-01）
- `CoreRestartTracker` と `FailureClock` 抽象を `actor-core` に導入し、`RestartStatistics` がコア実装を利用するよう更新済み。
- `TokioFailureClock` でアンカー更新を行い、既存の `Instant` ベース API（`push`/`with_values`）を維持したまま no_std 互換化。
- 旧 `Instant` ベース実装は撤去済み。残タスクは window 最適化と embedded 向けクロック実装。
