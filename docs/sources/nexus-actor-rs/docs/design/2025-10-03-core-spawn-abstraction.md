# Core spawn 抽象に関するメモ（更新版）

当初は `CoreSpawn*` 系の新トレイトを追加して `tokio::spawn` / `embassy_executor::Spawner` を統一的に扱う計画だったが、現状のコードベースでは `nexus_actor_core_rs::Spawn` を活用することで課題を解決している。追加の CoreSpawner 抽象や JoinHandle 導入は現状不要なため、本ドキュメントで提示していた設計案と TODO は撤去した。もし再び executor 依存の差分が問題化した場合は、最新の `Spawn` 実装状況と既存ミドルウェア（sender/spawn）を確認した上で再検討すること。
