# sender/spawn middleware Core ブリッジメモ（更新版）

以前のメモでは `CoreSpawn*` 系の新抽象を導入する計画を記載していたが、現行コードベースでは `nexus_actor_core_rs::Spawn` と既存の spawn ミドルウェアで課題を解消済み。追加の CoreSpawn ブリッジは不要となったため、詳細設計と TODO を撤去した。今後 sender や spawn の経路で問題が再発した場合は、現行 `Spawn` トレイトの活用状況を確認してから新規抽象の検討に進むこと。
