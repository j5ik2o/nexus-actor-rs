# utils 分割計画メモ (2025-10-01)

## 区分基準
- **目的**: no_std + alloc 対応の第一歩として utils を core / std に分離する。
- **現状把握**: どこが std 依存になっているか、どのクレートが影響を受けるかを棚卸し。
- **作業ステップ**: Codex Exec (被対話モード) に依頼しやすい単位での実施手順。
- **チェックリスト**: 完了判定に必要な確認項目。

## 目的
- `nexus-utils-core-rs` を no_std + alloc 前提の最小ユーティリティ層に縮小し、std / tokio 依存部分を別クレートへ切り出す。
- 将来的に `nexus-actor-core-rs` などを no_std 化するための前提条件を整える。

## 現状把握
- `modules/utils/src` の大半が `std::sync::{Arc, Mutex}`, `std::fmt` に依存。
  - `rg "std::" modules/utils/src` でヒットする箇所を一覧取得済み。
  - データ構造 (`queue`, `ring_queue`, `priority_queue`) や同期プリミティブ (`WaitGroup`, `AsyncBarrier`) で std 前提の API を利用。
- `nexus-actor-core-rs`, `nexus-remote-core-rs` などが utils に依存しているため、現段階では `cargo test --no-default-features` が通らない。

## 作業ステップ（Codex Exec に渡す想定）
1. **std 依存の棚卸し**
   - コマンド: `rg "std::" modules/utils/src`
   - 結果をメモし、どのモジュールが no_std 対応できるか分類。
2. **新クレート雛形の作成**
   - `cargo new modules/utils-core --lib`
   - `Cargo.toml` で `#![no_std]` + `features = ["alloc"]` を定義。
   - README や lib.rs に想定 API をコメントで記述。
3. **既存クレートの改名**
   - 現在の `nexus-utils-core-rs` を `nexus-utils-std-rs` にリネーム。
   - 依存しているクレートの `Cargo.toml` を更新（`nexus-utils-std-rs` + `nexus-utils-core-rs`）。
4. **std 非依存コードを utils-core へ移植**
   - 例: `queue`, `ring_queue` などを `modules/utils-core` に移動。
   - `std` に依存する部分（`Arc`, `Mutex`）は `cfg(feature = "std")` または `utils-std` 側に保持。
5. **std 向け補助クレートの整備**
   - `modules/utils-std`（仮）で tokio / std 依存コードを re-export。
   - `nexus-utils-std-rs` が `nexus-utils-core-rs` を feature 付きで利用できるようにする。
6. **依存元の調整**
   - `modules/actor-core/Cargo.toml` 等を更新し、core と std の両方を適切に参照。
   - `cargo test --workspace` & `cargo test --no-default-features --features alloc -p nexus-utils-core-rs` を実行。
7. **CI への追加**
   - `.github/workflows/ci.yml` に no_std ビルドを追加（`cargo check -p nexus-utils-core-rs --no-default-features --features alloc`）。

## 現状整理（区分: 状態 / 影響 / 残課題）
- **状態**: `modules/utils-core` に `RingBuffer` を実装し、`RingQueue` は std 層で `Mutex<RingBuffer>` をラップする構造へ移行済み。加えて MPSC／Ring／Priority 各キューを `CoreMailboxQueue` に直結するアダプタ（`UnboundedMpscCoreMailboxQueue` など）を導入。
- **影響**: キュー操作ロジックは core に集約され、`nexus-utils-std-rs` は同期プリミティブ（`Mutex`/`Arc`）と Tokio 依存コードのみを保持。Mailbox 側は trait object 化された `CoreMailboxQueue` を経由し、メトリクス取得が統一化。
- **残課題**: なし（2025-10-01 時点）。今後は MPSC 以外の同期プリミティブ微調整やキュー latency トラッキングの拡張が対象。

## チェックリスト
- [x] `nexus-utils-core-rs` が `#![no_std]` でコンパイルできる。
- [x] `nexus-utils-std-rs` に std / tokio 依存コードが集約されている（`QueueError`/`QueueSize`/`PriorityMessage` は core へ移動済み）。
- [x] 既存クレート（`actor`, `remote`, `cluster`）が新構成でテストをパス。
- [x] CI に no_std チェックを追加し、bench / publish ワークフローへ追従変更が不要であることを確認（2025-10-01）。

## 参考ドキュメント
- `docs/design/2025-09-30-migration-plan.md`
- 現状の `modules/utils/Cargo.toml`
- このメモ: `docs/worknotes/2025-10-01-utils-split.md`
