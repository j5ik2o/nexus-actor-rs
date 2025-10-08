# remote-core embedded 連携メモ（更新版）

旧ドキュメントでは `CoreSpawner` や新しい `RemoteRuntime` 抽象を前提とした API 再設計を検討していたが、最新コードではまだ `Spawn` 抽象と既存の remote 実装をベースに改修を進めている。未実装の API に関する手順は撤去し、今後の検討事項としては「remote-core を no_std でビルド可能にする」「Embassy 向けトランスポートの PoC を作成する」といった具体的な作業に絞ること。
