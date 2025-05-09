# Nexus Actor RS: coreモジュールリファクタリングTODO

このファイルには、coreモジュールの構造的複雑さを改善するためのTODOリストが含まれています。各TODOは独立したステップとして実装でき、段階的にリファクタリングを進めることができます。

## フェーズ1: 準備と分析

### TODO: 依存関係の可視化
```rust
// TODO: モジュール間の依存関係を分析し、依存グラフを作成する
// - 現在のモジュール構造を図示する
// - 循環依存を特定する
// - リファクタリングの優先順位を決定する
```

### TODO: テスト環境の整備
```rust
// TODO: 既存のテストが正常に動作することを確認し、テストカバレッジを測定する
// - すべてのテストが通ることを確認
// - テストカバレッジを測定し、不足している部分を特定
// - テスト実行時間を測定し、最適化の余地を特定
```

## フェーズ2: テストコードの分離

### TODO: テストディレクトリ構造の作成
```rust
// TODO: core/tests/ ディレクトリに適切な構造を作成する
// - actor_tests/ ディレクトリを作成
// - message_tests/ ディレクトリを作成
// - middleware_tests/ ディレクトリを作成
// - integration_tests/ ディレクトリを作成
```

### TODO: 単体テストの移行
```rust
// TODO: 各実装ファイルから単体テストを分離し、#[cfg(test)] モジュールに移動する
// - actor/actor/actor_behavior_test.rs のテストを actor/actor/actor_behavior.rs 内の #[cfg(test)] モジュールに移動
// - actor/actor/pid_set_test.rs のテストを actor/actor/pid_set.rs 内の #[cfg(test)] モジュールに移動
// - 他の *_test.rs ファイルも同様に処理
```

### TODO: 統合テストの移行
```rust
// TODO: 統合テストを core/tests/ ディレクトリに移動する
// - actor_system_test.rs を core/tests/actor_tests/ に移動
// - interaction_test.rs を core/tests/integration_tests/ に移動
// - 他の統合テストも適切なディレクトリに移動
```

## フェーズ3: 基本モジュールのリファクタリング

### TODO: utils モジュールの整理
```rust
// TODO: 汎用的なユーティリティ関数を core/src/utils/ に移動する
// - collections 関連のユーティリティを utils/collections.rs に移動
// - 時間関連のユーティリティを utils/time.rs に移動
// - 文字列操作関連のユーティリティを utils/strings.rs に移動
```

### TODO: message モジュールの再構築
```rust
// TODO: core/src/message/ ディレクトリを作成し、メッセージ関連のコードを移動する
// - actor/message/ ディレクトリの内容を core/src/message/ に移動
// - message.rs を message/base.rs に名前変更
// - message_headers.rs を message/headers.rs に名前変更
// - message_batch.rs を message/batch.rs に名前変更
// - auto_receive_message.rs を message/system.rs に統合
```

### TODO: process モジュールの再構築
```rust
// TODO: core/src/process/ ディレクトリを作成し、プロセス関連のコードを移動する
// - actor/process/ ディレクトリの内容を core/src/process/ に移動
// - process_registry.rs を process/registry.rs に名前変更
// - actor/actor/actor_process.rs を process/actor_process.rs に移動
```

## フェーズ4: アクターモジュールのリファクタリング

### TODO: アクター基本定義の整理
```rust
// TODO: core/src/actor/base.rs を作成し、基本的なアクタートレイトと実装を移動する
// - actor/actor/actor.rs の Actor トレイト定義を actor/base.rs に移動
// - actor/actor/actor_behavior.rs の内容を actor/base.rs に統合
// - actor/actor/actor_error.rs の内容を actor/base.rs に統合
// - actor/actor/actor_handle.rs の内容を actor/base.rs に統合
```

### TODO: コンテキスト関連コードの整理
```rust
// TODO: core/src/actor/context.rs を作成し、コンテキスト関連のコードを移動する
// - actor/context/actor_context.rs の内容を actor/context.rs に移動
// - actor/context/context_handle.rs の内容を actor/context.rs に統合
// - actor/context/actor_context_extras.rs の内容を actor/context.rs に統合
// - actor/context/state.rs の内容を actor/context.rs に統合
```

### TODO: ライフサイクル関連コードの整理
```rust
// TODO: core/src/actor/lifecycle.rs を作成し、ライフサイクル関連のコードを移動する
// - actor/actor/actor.rs のライフサイクルメソッド (pre_start, post_start など) を actor/lifecycle.rs に移動
// - actor/actor/restart_statistics.rs の内容を actor/lifecycle.rs に統合
// - actor/context/actor_context.rs のライフサイクル関連メソッドを actor/lifecycle.rs に移動
```

### TODO: スーパービジョン関連コードの整理
```rust
// TODO: core/src/actor/supervision.rs を作成し、スーパービジョン関連のコードを移動する
// - actor/supervisor/ ディレクトリの内容を actor/supervision.rs に統合
// - strategy_one_for_one.rs, strategy_all_for_one.rs の内容を統合
// - supervision_event.rs の内容を統合
// - directive.rs の内容を統合
```

## フェーズ5: ディスパッチとミドルウェアのリファクタリング

### TODO: ディスパッチ関連コードの整理
```rust
// TODO: core/src/dispatch/ ディレクトリを作成し、ディスパッチ関連のコードを移動する
// - actor/dispatch/ ディレクトリの内容を core/src/dispatch/ に移動
// - mailbox.rs, mailbox_message.rs, mailbox_producer.rs を dispatch/mailbox.rs に統合
// - future.rs を dispatch/future.rs に移動
// - bounded.rs, unbounded.rs を dispatch/channels.rs に統合
```

### TODO: ミドルウェア関連コードの整理
```rust
// TODO: core/src/middleware/ ディレクトリを作成し、ミドルウェア関連のコードを移動する
// - actor/actor/middleware.rs, actor/actor/middleware_chain.rs を middleware/base.rs に統合
// - actor/actor/sender_middleware.rs, actor/actor/sender_middleware_chain.rs を middleware/sender.rs に統合
// - actor/actor/receiver_middleware.rs, actor/actor/receiver_middleware_chain.rs を middleware/receiver.rs に統合
// - actor/actor/spawn_middleware.rs を middleware/spawn.rs に移動
```

## フェーズ6: 型付きアクターのリファクタリング

### TODO: 型付きアクター関連コードの整理
```rust
// TODO: core/src/actor/typed.rs を作成し、型付きアクター関連のコードを移動する
// - actor/actor/typed_actor.rs の内容を actor/typed.rs に移動
// - actor/actor/typed_actor_handle.rs の内容を actor/typed.rs に統合
// - actor/actor/typed_actor_producer.rs の内容を actor/typed.rs に統合
// - actor/actor/typed_actor_receiver.rs の内容を actor/typed.rs に統合
// - actor/actor/typed_pid.rs の内容を actor/typed.rs に統合
// - actor/actor/typed_props.rs の内容を actor/typed.rs に統合
// - actor/context/typed_context_handle.rs の内容を actor/typed.rs に統合
// - actor/context/typed_root_context.rs の内容を actor/typed.rs に統合
```

## フェーズ7: 公開APIの整理

### TODO: lib.rs の再構築
```rust
// TODO: core/src/lib.rs を更新し、新しいモジュール構造を反映する
// - 新しいモジュール構造に合わせて pub mod 宣言を更新
// - 非公開モジュールは mod のみで宣言
// - 必要に応じて re-export を追加
```

### TODO: 公開APIの整理
```rust
// TODO: core/src/api.rs を作成し、公開APIを整理する
// - 公開すべきAPIを api モジュールにまとめる
// - lib.rs から pub use api::* でエクスポート
// - 内部実装の詳細は直接エクスポートしない
```

## フェーズ8: クリーンアップと最適化

### TODO: 未使用コードの削除
```rust
// TODO: 未使用のコードや重複したコードを特定し削除する
// - #[allow(dead_code)] 属性を確認し、実際に使用されていないコードを削除
// - 重複した機能を提供するコードを統合
// - 非推奨となった機能を削除または非推奨としてマーク
```

### TODO: インポートパスの最適化
```rust
// TODO: インポートパスを最適化し、一貫性を確保する
// - 相対パスと絶対パスの使用を統一
// - use 文をアルファベット順に並べ替え
// - グループ化できる use 文をグループ化
// - 不要な use 文を削除
```

### TODO: ドキュメントの更新
```rust
// TODO: コードドキュメントを更新し、新しい構造を反映する
// - 各モジュールにモジュールレベルのドキュメントを追加
// - 公開APIにはすべてドキュメントコメントを追加
// - 例を更新して新しい構造を反映
// - README.mdを更新して新しい構造を説明
```

## 実装のガイドライン

各TODOを実装する際は、以下のガイドラインに従ってください：

1. **1つのTODOごとにブランチを作成**: 各TODOは独立した作業単位として実装し、個別のブランチで管理します。
2. **テストを維持**: リファクタリング中もすべてのテストが通ることを確認します。
3. **コミットメッセージ**: 「TODO: XXXを実装」という形式でコミットメッセージを記述します。
4. **レビュー**: 各TODOの実装後にコードレビューを行います。
5. **マージ**: レビュー後、問題がなければメインブランチにマージします。

## 優先順位

TODOの実装順序は、基本的にリストされた順序に従いますが、以下の依存関係に注意してください：

1. フェーズ1と2（準備とテスト分離）は他のリファクタリングの前に完了させる
2. 基本モジュール（utils, message, process）は他のモジュールより先にリファクタリングする
3. アクターモジュールのリファクタリング（フェーズ4）は基本モジュールのリファクタリング後に実施する
4. 公開APIの整理（フェーズ7）は他のすべてのリファクタリングが完了した後に実施する
