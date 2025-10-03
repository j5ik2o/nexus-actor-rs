# sender/spawn middleware Core ブリッジ設計メモ（2025-10-03）

## 区分基準
- **課題整理**: 現状の制約と原因を明確化する。
- **設計方針**: Core 側へ橋渡しするための抽象設計を示す。
- **実装ステップ**: 実際の変更手順を段階的に分解する。

## 課題整理
1. **SenderContextHandle の復元不能**  
   - `SenderContextHandle` は `ContextHandle`/`RootContext` を内部に保持し、実行時に `ActorSystem` へアクセスする。  
   - `CoreSenderInvocation` には ActorSystem 参照が無いため、Core → std へ戻した際に送信 API を呼べない。
2. **SpawnMiddleware が std 依存**  
   - `SpawnMiddleware` は `Spawner` を引数・戻り値に取るが、`Spawner` は `ActorSystem`／`Props`／`SpawnerContextHandle` へ依存。  
   - CoreProps に `Spawner` を埋め込んでも no_std 環境で実行できず、Core 実装と整合しない。

## 設計方針
1. **CoreSenderSnapshot の導入**  
   - `CoreActorContextSnapshot` に加え、送信時に必要なメタ情報（例: `ActorSystemId`、カスタムハンドル識別子）を保持する純データ構造を新設する。  
   - Core 環境ではデータのまま保持し、std 実行時に `SenderContextFactory` が ActorSystem を解決して `SenderContextHandle` を生成する。
2. **CoreSenderMiddlewareChain の拡張**  
   - `CoreSenderInvocation` を `(CoreSenderSnapshot, CorePid, CoreMessageEnvelope)` に拡張。  
   - std 側は `SenderContextFactory`（`Arc<dyn Fn(CoreSenderSnapshot) -> SenderContextHandle>`）を tail クロージャとして登録し、実行時に Snapshot からハンドルを復元する。
3. **CoreSpawnInvocation と Factory**  
   - 親コンテキスト snapshot、`CoreProps`, 子アクター ID などを保持する `CoreSpawnInvocation` を定義。  
   - std 側で `SpawnContextFactory` や `SpawnerBridge` を用意し、CoreInvocation から既存 `Spawner` 呼び出しへ再ルーティングする。
4. **Props::rebuild_core_props の整理**  
   - sender/spawn の Core チェーンを作成する処理を追加し、CoreProps へセットする。  
   - 工廠時には tail クロージャが ActorSystem／Context を取得できるよう `ActorSystemBridge` をコールバックとして登録。

## 実装ステップ
1. Core クレート
   1. `CoreSenderSnapshot`・`CoreSpawnInvocation` を追加。  
   2. `CoreSenderInvocation` を snapshot を含む構造へ更新し、`CoreSenderMiddlewareChainHandle` を再定義。  
   3. `CoreProps` に sender/spawn middleware 用のフィールドと setter/getter を追加。
2. std クレート
   1. `SenderContextHandle` に `from_core_snapshot`（`ActorSystem` を引数に受ける）を追加。  
   2. `SenderMiddlewareChain::to_core_invocation_chain` を実装し、CoreSnapshot → std 実行を復元するブリッジを追加。  
   3. `SpawnMiddleware` に `to_core_invocation_chain`（`SpawnerBridge`）を追加。  
   4. `Props::rebuild_core_props` の sender/spawn セクションを実装。  
   5. `RootContext` など sender middleware を利用する箇所が新チェーン API で動作するよう順次調整。
3. テスト
   - sender/spawn middleware を利用するユニットテストを追加し、CoreProps へ伝播後も従来通り動作することを検証。  
   - Integration テストで CoreProps を経由した監視チェーンが正しく再生されるか確認。


## 実装ステップ詳細 (2025-10-03 更新)

- ContextRegistry を追加:
  - ActorSystemId と CorePid の組をキーに Weak Context を管理。
  - ContextHandle/RootContext がスコープに入る際に登録、ドロップ時に解除。
- Sender ブリッジ:
  - CoreSenderSnapshot に ActorSystemId/self_pid 等を含める。
  - SenderContextHandle::from_core_snapshot() で ActorSystemRegistry + ContextRegistry 経由で復元。
  - SenderMiddlewareChain::to_core_invocation_chain() で CoreInvocation -> std 実行を行う。
- Spawn ブリッジ:
  - CoreSpawnInvocation (親 snapshot, child CoreProps, child pid, metadata) を定義。
  - SpawnMiddleware::to_core_invocation_chain() で std props/spawner へ戻す。
  - Props::from_core_props() で CoreProps -> std Props を復元。
- Props::rebuild_core_props() を更新し、sender/spawn チェーンも CoreProps に反映。
- 既存ユニットテストを拡張し、ContextRegistry なしでは動作しないケースも検証。
