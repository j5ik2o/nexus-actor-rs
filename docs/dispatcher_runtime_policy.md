# Dispatcher Runtime ポリシー (2025-09-29)

## ゴール
1. Runtime を内部保持する Dispatcher で安全に shutdown できること。
2. 追加の Dispatcher 実装でも同じパターン（`Option<Arc<Runtime>>` + `shutdown_background`）を踏襲すること。

## 現況
- `SingleWorkerDispatcher` は `Drop` 時に `shutdown_background()` を呼ぶ実装へ移行済み。
- 既存 Dispatcher を利用する ActorSystem / Remote テストでも Runtime の二重 drop は発生していない。

## TODO
- Runtime を内部管理する新規 Dispatcher を追加する際は、このポリシーを参照してパターンを統一する。
- ランタイム不要な Dispatcher (`CurrentThreadDispatcher` など) との API 平準化は別タスクで検討する。

