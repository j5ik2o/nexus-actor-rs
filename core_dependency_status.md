# nexus-actor-rs 循環依存解消作業の現状

作成日: 2025/01/27

## 実施した作業

1. **traitsモジュールの作成**
   - `core/src/traits.rs` と `core/src/traits/` ディレクトリを作成
   - `actor_traits.rs`: Actorトレイトを移動
   - `context_traits.rs`: Context関連のトレイトを定義
   - `handle_traits.rs`: ContextHandleの実装を移動

2. **インポートの更新**
   - `actor/core/actor.rs`: Actorトレイトを再エクスポート
   - `actor/context.rs`: traitsモジュールからトレイトを再エクスポート
   - `actor/context/context_handle.rs`: ContextHandleを再エクスポート

## 現在の問題

### 1. 大量のビルドエラー（53個）
- トレイトの実装が不足
- メソッドシグネチャの不一致
- 型制約の問題

### 2. 設計上の課題
- Contextトレイトが多数のサブトレイトを要求する複雑な構造
- トレイトオブジェクトとしての使用時にDebugトレイトが実装されていない
- BasePart、OtherPartなど、責務が不明確なトレイトの存在

### 3. 循環依存の根本原因
- ActorがContextHandleに依存
- ContextHandleがContextトレイトに依存
- ContextトレイトがActorに関連する型（ExtendedPid、Props等）に依存

## 推奨される解決策

### オプション1: 段階的なリファクタリング（推奨）
1. 現在の変更を一旦ロールバック
2. より小さな単位で循環依存を解消
3. 各ステップでテストが通ることを確認

### オプション2: インターフェースの再設計
1. Context関連のトレイトを簡素化
2. 必要最小限のメソッドのみを持つ基本トレイトを定義
3. 拡張機能は別のトレイトとして定義

### オプション3: 依存性逆転の原則を適用
1. 抽象的なメッセージハンドラートレイトを定義
2. ActorとContextを同じ抽象化レベルに配置
3. 具体的な実装を別モジュールに分離

## 次のステップ

1. 現在の変更をロールバックするか、エラーを一つずつ修正するか決定
2. より詳細な依存関係図を作成
3. 最小限の変更で循環依存を解消する方法を検討