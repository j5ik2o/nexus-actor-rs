# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

MUST: 必ず日本語で応対すること

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
```

## アーキテクチャ概要

### ワークスペース構成

1. **utils** - 共通ユーティリティライブラリ
   - コレクション（DashMap拡張、Queue、Stack）
   - 並行処理プリミティブ（AsyncBarrier、CountDownLatch、WaitGroup）
   - 同期化ユーティリティ

2. **message-derive** - メッセージ処理用の手続きマクロ
   - アクターメッセージの自動実装
   - タイプセーフなメッセージハンドリング

3. **core** - アクターシステムのコア実装
   - アクターシステム、コンテキスト、ライフサイクル管理
   - メールボックスとディスパッチャー
   - スーパーバイザー戦略（OneForOne、AllForOne、Restarting）
   - イベントストリーム
   - プロセスレジストリ
   - メトリクス統合（OpenTelemetry）

4. **remote** - リモートアクター機能
   - gRPC/Tonicベースの通信
   - エンドポイント管理
   - リモートプロセス処理
   - シリアライゼーション

5. **cluster** - クラスタリングサポート
   - ゴシッププロトコル
   - メンバー状態管理
   - 分散アクターのサポート

### 主要な設計パターン

1. **アクターモデル**
   - 各アクターは独立したプロセスとして動作
   - メッセージパッシングによる通信
   - 状態の隔離とスレッドセーフティ

2. **スーパーバイザー階層**
   - 親子関係によるアクターの管理
   - 障害時の自動復旧戦略
   - 指数バックオフによるリトライ

3. **メールボックスシステム**
   - 有界/無界のメッセージキュー
   - 優先度付きメッセージ処理
   - バックプレッシャー制御

4. **タイプセーフなメッセージング**
   - 型付きPIDとアクター
   - コンパイル時の型チェック
   - derive マクロによる自動実装

### Protocol Buffers

プロジェクトは通信とシリアライゼーションにProtocol Buffersを使用：
- `proto/` ディレクトリに定義ファイル
- `build.rs` でコード生成
- `generated/` に生成されたコード

## 開発時の注意点

1. **非同期ランタイム**: Tokioをフル機能で使用
2. **エラーハンドリング**: アクターの障害は親に伝播し、スーパーバイザー戦略で処理
3. **テスト構成**:
   - **単体テスト**: 実装の横に配置（例: `actor_context.rs` の隣に `actor_context/tests.rs`）
   - **結合テスト**: `tests/` ディレクトリにのみ配置
   - これは重要な規約なので必ず守ること
4. **モジュール構成**:
   - **Rust 2018エディション**: `mod.rs`は使用禁止
   - モジュールと同名のファイルを使用（例: `foo.rs` と `foo/` ディレクトリ）
   - これは重要な規約なので必ず守ること
5. **メトリクス**: OpenTelemetryによる観測性の実装

## 例

`core/examples/` ディレクトリに様々なアクターパターンの実装例：
- actor-hello-world: 基本的なアクター
- actor-supervision: スーパーバイザー戦略
- actor-request-response: リクエスト/レスポンスパターン
- actor-parent-child: 親子アクター階層
- actor-dead-letter: デッドレター処理