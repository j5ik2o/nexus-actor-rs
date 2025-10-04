# PidSet 同期化メモ (2025-09-29 時点)

## 区分基準
- **完了事項**: 同期化移行で実施済みの内容。
- **フォローアップ**: 追加検討が必要なポイント。
- **リスク**: 現状の懸念事項。

## 完了事項
- `PidSet` は `parking_lot::RwLock` ベースの同期実装へ移行し、すべての API (`add` / `remove` / `contains` / `len` / `to_vec`) が同期メソッドになった（`modules/actor-core/src/actor/core/pid_set.rs`）。
- `PidSet::new_with_pids` を追加し、初期値付き生成が容易になった。テストは同期 API へ書き換え済み（`modules/actor-core/src/actor/core/pid_set/tests.rs`）。
- `ActorContextExtras` / `EndpointWatcher` など `PidSet` 利用箇所は同期 API 前提で同期ロック構造に統一。

## フォローアップ
- `PidSet::for_each` / `get` の利用箇所を棚卸しし、不要なクローンを削減する（大規模 Clone の削減）。
- 旧 async API を利用していた外部コード向けに互換レイヤを提供するか検討（現在は削除済み）。
- `PidSet` のベンチマークを追加し、ActorContext の子アクター管理におけるロック時間を継続測定する。

## リスク
- 同期化に伴う `parking_lot` 依存が `no_std` 対応の障壁になる可能性。必要に応じて抽象インターフェースを準備する。
- 大量 PID を扱うシナリオでは `Vec` + `HashMap` のコピーコストが残るため、`IndexSet` などの採用検討を継続。

