# core ライフタイム移行メモ (2025-09-29)

## ゴール
- `ActorContext` / `ContextHandle` の同期化 PoC を本格導入する際の要点をまとめる。

## 現況
- Context 周りの同期化 PoC は概ね完了し、主要な API は `ContextBorrow` と `ContextSnapshot` で同期参照が可能。
- Supervisor/metrics まわりも `ArcSwap` ベースの取得に切り替わっている。

## 今後の検討
- Virtual Actor 経路での `ContextHandle` 同期 API の再確認。
- `loom` などを用いた並行検証の必要性が出た際に、個別でタスク化する。

