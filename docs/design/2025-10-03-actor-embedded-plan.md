# actor-embedded クレート設計メモ (2025-10-03)

## 区分基準
- **構成**: クレート階層と feature 設計を明確化。
- **依存**: Embassy 系クレートと既存 nexus-* クレートの関係を整理。
- **タスク**: 実装着手に向けた MUST/SHOULD を列挙。

## 構成
- クレート名は `nexus-actor-embedded-rs` を想定し、`modules/actor-embedded` に配置。
- Cargo feature: `default = ["embassy"]` とし、`embassy` は `embassy-time`, `embassy-sync`, `embassy-executor` を有効化。`std` フラグは持たず strictly no_std。
- 公開 API: `pub use nexus_actor_core_rs::{actor::*, context::*, supervisor::*};` に加え、Embassy 向けにタイマ／スケジューラ／スポーナーを注入できる `EmbeddedRuntimeBuilder` を提供（`with_timer`/`with_scheduler`/`with_spawner` と `runtime::FnCoreSpawner`/`FnJoinHandle`）。
- `embedded_runtime` モジュールはデフォルトで `EmbassyTimerWrapper` と `UnsupportedScheduler` を用意し、利用側が Embassy の `Spawner` をラップした `CoreSpawner` を `with_spawner` で設定する想定。
- `AsyncMutex`/`AsyncNotify`/`AsyncRwLock` の暫定実装は `modules/utils-embedded` に配置し、必要に応じて最適化を進める。

## 依存
- 直接依存
  - `embassy-sync = { version = "*", default-features = false, features = ["defmt"]? }`（最小必要機能のみ選定）。
  - `embassy-time = { version = "*", default-features = false }`。
  - `embassy-executor = { version = "*", default-features = false }`（`Spawner` を利用）。
  - `nexus-actor-core-rs` / `nexus-utils-core-rs`（既存 no_std 抽象）。
- 開発/テスト依存
  - `embassy-futures`（`yield_now` のみ利用予定）
  - `defmt` はオプション扱い。`feature = "defmt-log"` で有効化する案。

## タスク（優先度順）
1. **MUST** `EmbeddedRwLock` の API 設計: actor-core が要求する read/write 操作（複数 read + 単一 write）を満たすか検証し、性能面の懸念があれば actor-core 側の利用箇所を要調整として洗い出す。→ 2025-10-04 時点で `embassy_sync::rwlock::RwLock` ベースの `EmbeddedRwLock` を実装し、複数 read / 単一 write のユニットテストを追加済み。
2. **MUST** `CoreSpawn` 抽象: executor 非依存でタスクを起動するトレイトを `actor-core` に追加し、Tokio (`tokio::spawn`) と Embassy (`Spawner::spawn`) 双方の実装方針を明示。
3. **MUST** `CoreScheduler` 再設計: Embassy でのタイマー駆動タスクを成立させる API（`schedule_once`/`schedule_repeated`）の内部実装案をまとめ、Tokio 側との差分と API 互換性を確認。
4. **SHOULD** ロギング Hook: `actor-core` で `CoreLogger`（仮称）を定義し、std/embedded で `tracing`/`defmt` を注入できる仕組みを検討。
5. **SHOULD** 一貫性テスト: `actor-embedded` の Mutex/RwLock/Notify を使った簡易ユニットテストを整備し、`cargo test -p nexus-utils-embedded-rs --no-default-features --features embassy` が通るまで整備。
6. **COULD** Embassy ベンチ: `examples/embedded_blink.rs` 等を用意し、`embassy_executor::run` + actor シナリオを動作確認する。

## メモ
- Embassy の `Mutex` は `Send` ガードを保証しないため、`unsafe impl Send for Guard` が必要な場合は安全性検証を別途行う。
- `CoreSpawner` は利用側から注入する想定（`EmbeddedRuntimeBuilder::with_spawner`）。Embassy 実装では `embassy_executor::Spawner` をラップしたアダプタを別途用意する。
- `CoreScheduler::schedule_repeated` 実装では `EmbassyScheduler` 内部にタスク記述子を保持し、キャンセル時に `Signal` へ通知する方式を検討する。
- `Cargo.toml` のバージョン pin は将来 Embassy 側の変更に追随しやすいよう `workspace = true` を活用する。
