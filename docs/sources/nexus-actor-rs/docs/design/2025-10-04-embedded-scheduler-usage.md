# Embassy Scheduler 利用メモ（更新版）

旧版では `EmbassyCoreSpawner` や `EmbeddedRuntimeBuilder::with_spawner` など、まだ実装されていない API を前提にした手順を掲載していた。現時点の `modules/actor-embedded` では `ImmediateSpawner` と `ImmediateTimer` を提供するに留まり、Embassy 向けの統合スケジューラ／spawner は未提供であるため、以前の手順を撤去した。Embassy 連携を再開する際は、最新の `spawn`/`timer` 実装と `actor-core` の `Spawn` 抽象を確認しつつ、新しい API 設計を再検討すること。
