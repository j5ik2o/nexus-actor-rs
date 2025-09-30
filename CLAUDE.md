# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

MUST: 必ず日本語で応対すること

## 重要な注意事項

- **応対言語**: 必ず日本語で応対すること
- **タスクの完了条件**: テストはすべてパスすること
- **テストの扱い**: 行うべきテストをコメントアウトしたり無視したりしないこと
- **実装方針**:
  - 既存の多くの実装を参考にして、一貫性のあるコードを書くこと
  - protoactor-go(@docs/sources/protoactor-go)の実装を参考にすること（Goの実装からRustイディオムに変換）
- **後方互換性**: 後方互換は不要（破壊的変更を恐れずに最適な設計を追求すること） 
- serena mcpを有効活用すること
- **ドキュメント方針**: すべてのドキュメントは MECE を意識して構成し、節や箇条書きの区分が重複・抜け漏れのないようにすること。

## プロジェクト概要

nexus-actor-rsは、Rustで実装されたアクターモデルライブラリです。分散システムにおける並行処理とメッセージパッシングを安全かつ効率的に実現します。

## ビルドとテストコマンド

### 基本的なビルドコマンド
```bash
# ワークスペース全体のビルド
cargo build

# リリースビルド
cargo build --release

# 特定のパッケージのビルド
cargo build -p nexus-actor-core-rs
```

### テストコマンド
```bash
# ワークスペース全体のテスト実行
cargo test --workspace

# 特定のテストを実行
cargo test test_name

# 特定のパッケージのテスト
cargo test -p nexus-actor-core-rs

# テストを並列実行せずに実行（デバッグ時に有用）
cargo test -- --test-threads=1

# 単一のテストファイルを実行
cargo test --lib actor::core::tests

# verboseモードでテストを実行（詳細な出力）
cargo test -- --nocapture
```

### 開発用コマンド
```bash
# コードフォーマット（nightlyツールチェーンを使用）
cargo make fmt

# 依存関係をアルファベット順にソート
cargo make sort-dependencies

# コードカバレッジレポートの生成
cargo make coverage
# または
./coverage.sh

# clippy（リント）の実行
cargo clippy --workspace -- -D warnings

# ドキュメントの生成と表示
cargo doc --open

# 特定のサンプルを実行
cargo run --example actor-hello-world
```

## アーキテクチャ概要

### ワークスペース構成

1. **utils** - 共通ユーティリティライブラリ
   - コレクション（DashMap拡張、Queue、Stack）
   - 並行処理プリミティブ（AsyncBarrier、CountDownLatch、WaitGroup、Synchronized）
   - 同期化ユーティリティ

2. **message-derive** - メッセージ処理用の手続きマクロ
   - `#[derive(Message)]` マクロの提供
   - アクターメッセージの自動実装
   - タイプセーフなメッセージハンドリング

3. **core** - アクターシステムのコア実装
   - **actor/core_types**: 独立した型定義（Actor trait サポートのみに整理済み）
   - **actor/core**: 従来のアクター実装（Actor trait、PID、Props）
   - **actor/context**: コンテキスト実装（ActorContext、生命周期管理）
   - **actor/dispatch**: メールボックスとディスパッチャー
   - **actor/supervisor**: スーパーバイザー戦略（OneForOne、AllForOne、Restarting）
   - **event_stream**: イベントストリーミング機能
   - **metrics**: OpenTelemetryによるメトリクス統合

4. **remote** - リモートアクター機能（⚠️ 開発中 - 本番利用非推奨）
   - gRPC/Tonicベースの通信層
   - エンドポイント管理（EndpointReader、EndpointWriter）
   - リモートプロセス処理（RemoteProcess）
   - Protocol Buffersによるシリアライゼーション
   - ブロックリスト管理
   - **現状**: 基本実装のみ、テスト不足、サンプル1つのみ

5. **cluster** - クラスタリングサポート（🚧 実装初期段階）
   - ゴシッププロトコルの実装
   - メンバー状態管理（GossipState、MemberStateDelta）
   - コンセンサスチェック機構
   - 分散アクターのサポート（予定）
   - **現状**: バージョン0.0.1、テスト未実装、サンプル未提供

### 主要な設計パターン

1. **アクターモデル**
   - 各アクターは独立したプロセスとして動作
   - メッセージパッシングによる非同期通信
   - 状態の隔離とスレッドセーフティの保証
   - タイプセーフなPID（TypedPid<T>）による型安全性

2. **スーパーバイザー階層**
   - 親子関係によるアクターの階層管理
   - 障害時の自動復旧戦略（Restart、Stop、Resume、Escalate）
   - 指数バックオフによるリトライ機構
   - RestartStatisticsによる障害履歴管理

3. **メールボックスシステム**
   - 有界（BoundedMailbox）/無界（UnboundedMailbox）のメッセージキュー
   - 優先度付きメッセージ処理
   - バックプレッシャー制御
   - デッドレター処理

4. **Actor トレイトの統一方針**
   - 2025/09/26 時点で BaseActor 系 API は削除済み
   - 以降は従来の `Actor` トレイトへ一本化
   - Props 経由の生成は既存 Actor トレイトで継続利用
   - Props 経由の生成は既存 Actor トレイトで継続利用

### Protocol Buffers

プロジェクトは通信とシリアライゼーションにProtocol Buffersを使用：
- `modules/proto/` ディレクトリに`.proto`定義ファイル
- `build.rs` でprostを使用したコード生成
- `generated/` ディレクトリに生成されたコード
- remote/clusterモジュールで活用

## 開発時の注意点

1. **非同期ランタイム**: Tokioをフル機能で使用（`features = ["full"]`）

2. **エラーハンドリング**:
   - アクターの障害は親に伝播し、スーパーバイザー戦略で処理
   - `ErrorReason`列挙型による構造化されたエラー表現
   - `ActorError`によるアクター固有のエラーハンドリング

3. **テスト構成**:
   - **単体テスト**: 実装の横に配置（例: `actor_context.rs` の隣に `actor_context/tests.rs`）
   - **結合テスト**: `tests/` ディレクトリにのみ配置
   - **`*_test.rs`ファイルは使用しない**（リファクタリング対象）
   - これは重要な規約なので必ず守ること

4. **モジュール構成**:
   - **Rust 2018エディション**: `mod.rs`は使用禁止
   - モジュールと同名のファイルを使用（例: `foo.rs` と `foo/` ディレクトリ）
   - サブモジュールは親モジュール名のディレクトリ内に配置
   - これは重要な規約なので必ず守ること

5. **メトリクス**:
   - OpenTelemetryによる観測性の実装
   - ActorMetricsによるアクター固有のメトリクス収集
   - ProtoMetricsによるProtocol Buffersメッセージのメトリクス

6. **循環依存の解決（PROJECT_STATUS.md参照）**:
   - 当初は BaseActor / BaseContext で一時回避
   - 現在は Actor トレイトへ再集約し、BaseActor 系アダプタを撤去済み
   - 既存コードは Actor トレイトで互換性を維持

## 例

`core/examples/` ディレクトリに様々なアクターパターンの実装例：
- **基本的な例**:
  - actor-hello-world: 最も基本的なアクター
  - actor-request-response: リクエスト/レスポンスパターン
  - actor-parent-child: 親子アクター階層

- **高度な例**:
  - actor-supervision: スーパーバイザー戦略のデモ
  - actor-dead-letter: デッドレター処理
  - actor-mailbox-middleware: メールボックスミドルウェア
  - actor-receive-middleware: 受信ミドルウェアの実装
  - actor-back-pressure: バックプレッシャー制御
  - actor-lifecycle-events: ライフサイクルイベント処理
  - actor-auto-respond: 自動応答パターン

- **レガシー移行サンプル**:
  - `core/examples/legacy/*` に旧 BaseActor 系サンプルを隔離（新規コードでは利用しない）

PROJECT_STATUS.mdに沿って作業を進めること

## モジュールの成熟度

### プロダクション利用可能
- **core** (v1.x): 安定版、85以上のテストでカバー、本番環境での利用実績あり
- **utils** (v0.x): 安定、coreで広く利用されている
- **message-derive** (v0.x): 安定、マクロは成熟している

### 開発中（注意が必要）
- **remote** (v0.1.x):
  - 基本的な機能は動作するが、エッジケースの処理が不完全
  - テストカバレッジが不十分（serializer.rsのみテストあり）
  - サンプルが1つのみ（remote-activate）
  - エラーハンドリングとリトライ機構の改善が必要

### 初期実装段階（本番利用非推奨）
- **cluster** (v0.0.1):
  - 基本的なスケルトンのみ実装
  - テストが一切存在しない
  - サンプルコードなし
  - 実際のクラスタリング機能は未実装
  - APIが大幅に変更される可能性が高い

## 今後の開発優先順位

1. **coreモジュールの継続的改善**
   - Actor トレイト前提での最適化・クリーンアップ
   - パフォーマンス最適化

2. **remoteモジュールの安定化**
   - 包括的なテストスイートの追加
   - エラーハンドリングの強化
   - ドキュメントとサンプルの充実

3. **clusterモジュールの実装**
   - 基本的なクラスタリング機能の実装
   - テストとサンプルの追加
   - ドキュメントの整備
