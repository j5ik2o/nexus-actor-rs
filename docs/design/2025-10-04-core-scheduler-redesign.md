# CoreScheduler 再設計検討 (2025-10-04)

## 区分基準
- **要件定義**: Tokio / Embassy 共通で満たすべき API 振る舞いの整理。
- **設計案**: インターフェース変更と内部コンポーネント分解の方針。
- **実装タスク**: 実コードへの反映ステップと優先順位。

## 要件定義
1. **時刻基準の統一**
   - `schedule_once` と `schedule_repeated` は *任意の Instant 提供* を前提としない。
   - Embassy (`embassy_time::Timer`) は `Duration` ベースで要求を満たせるが、Tokio 側でも `tokio::time::sleep_until` が必要になるケースを考慮。
2. **キャンセル / 状態取得**
   - ハンドルが `cancel()` 以外に `is_cancelled()` を返す API を最低限提供。
   - Embassy では `Signal`／`Alarm` を利用したキャンセルリング、Tokio では `JoinHandle::abort` 相当をラップ。
3. **停止処理**
   - ランタイム終了時にスケジュール済みタスクを一括停止できるよう `CoreSchedulerShutdown` 相当のフックを検討。
4. **割り込み安全性**
   - Embassy 環境の `no_std` 制約下でも `Sync + Send` を維持。

## 設計案
1. **API 変更**
   - `CoreScheduledHandle` に `fn is_active(&self) -> bool` を追加。
   - `CoreScheduler` に `fn drain(&self)` を追加し、全ハンドルの停止を実装者側へ委譲。
   - `CoreRuntimeConfig::with_scheduler` / `CoreRuntime::scheduler()` は従来通りだが、新設メソッドへ透過的アクセスできるよう `CoreScheduledHandleRef` を更新。
2. **Tokio 実装**
   - `nexus_utils_std_rs::runtime::TokioScheduler` を拡張し、`tokio::task::JoinHandle` + `tokio::time::Sleep` を利用したタイマーベースの内部管理 `ScheduledTask` を保持。
   - `drain` 呼び出しで `JoinHandle::abort` を起動。
3. **Embassy 実装**
   - `nexus_utils_embedded_rs` に `EmbassyScheduler` を新設。
   - `embassy_time::Timer::after` と `Signal` を組み合わせ、再スケジュール（`schedule_repeated`）時はループ Future を生成。
   - `drain` は各タスクにキャンセルフラグを立て `Signal` を発火。
4. **テスト戦略**
   - `cfg(feature = "std")` 下で `schedule_once` / `schedule_repeated` が所要時間を概ね満たす非同期テストを追加。
   - `no_std` モードではモックタイマを注入し、インスタントを疑似しながら `is_active` と `drain` が期待通り遷移することを確認。
   - CI に `cargo test --no-default-features --features embassy --no-run` を追加し、Embassy 実装が継続的にコンパイルされることを保証。
   - `modules/actor-embedded/examples/embassy_scheduler.rs` を継続的にビルドし、Embassy 向けサンプルが API の変更に追随できるようにする。

## 実装タスク
1. **インターフェース更新 (MUST)**
   - `CoreScheduler`, `CoreScheduledHandle`, `CoreScheduledHandleRef` の API 拡張。
   - 既存使用箇所 (`TokioScheduler`, `EmbeddedRuntimeBuilder`) のビルド修正。
2. **TokioScheduler 改修 (MUST)**
   - タスク管理構造の刷新と `drain` 実装。
   - ユニットテスト追加 (`tokio::test`)。
3. **EmbassyScheduler 実装 (MUST)**
   - `EmbassyTaskSlot` を共用しつつ、タイマーベース Future を生成するヘルパーを追加。
   - `cargo test --no-default-features --features embassy --no-run` でコンパイル検証。
4. **ドキュメント更新 (SHOULD)**
   - `docs/design/2025-10-03-actor-embedded-plan.md` と `docs/remote_improvement_plan.md` を再更新し、API 変更点と移行手順を追記。
5. **後続検討 (COULD)**
   - 将来的な `schedule_at(Instant)` の導入可否を検討。
