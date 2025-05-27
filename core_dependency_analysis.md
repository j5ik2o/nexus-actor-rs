# nexus-actor-rs coreモジュール依存関係分析

作成日: 2025/01/27

## 概要

このドキュメントは、nexus-actor-rsのcoreモジュールにおける依存関係の分析結果をまとめたものです。
リファクタリングのフェーズ1として実施しました。

## 現在のモジュール構造

```
core/
├── actor/                 # アクターシステムのコア実装
│   ├── actor_system/      # アクターシステムの管理
│   ├── context/          # アクターコンテキスト
│   ├── core/             # 基本型と振る舞い
│   ├── dispatch/         # メッセージディスパッチ
│   ├── event_stream/     # イベントストリーム処理
│   ├── message/          # メッセージ型定義
│   ├── process/          # プロセス管理
│   ├── supervisor/       # 監督戦略
│   ├── guardian.rs       # ガーディアンアクター
│   └── metrics/          # メトリクス
├── ctxext/               # コンテキスト拡張
├── event_stream/         # イベントストリーム実装
├── extensions/           # 拡張機能
├── generated/            # Protocol Buffers生成コード
└── metrics/              # メトリクス収集
```

## 主要な依存関係

### 1. 循環依存（最重要課題）

#### core ⇄ context の相互依存
```
core → context:
  - Actor, Props, ActorHandle が ContextHandle を使用
  
context → core:
  - ActorContext が Actor, Props, ExtendedPid を使用
```

この循環依存は、アーキテクチャ上の重大な問題です。

### 2. 依存関係の階層

```
┌─────────────────┐
│  actor_system   │ ← 最上位層
└────────┬────────┘
         │
┌────────┴────────┬─────────────┬──────────────┐
│    context      │   process   │   dispatch   │ ← 中間層
└────────┬────────┴──────┬──────┴──────┬───────┘
         │               │             │
┌────────┴────────┬──────┴──────┬──────┴───────┐
│      core       │   message   │  supervisor  │ ← 基礎層
└─────────────────┴─────────────┴──────────────┘
```

### 3. 複雑な依存関係を持つモジュール

#### actor_system が依存するモジュール（11個）：
- context (RootContext, TypedRootContext)
- core (ExtendedPid)
- dispatch (DeadLetterProcess)
- event_stream (EventStreamProcess)
- guardian (GuardiansValue)
- message (EMPTY_MESSAGE_HEADER)
- process (ProcessRegistry, ProcessHandle)
- supervisor (subscribe_supervision)
- metrics
- extensions
- generated

#### actor_context.rs の過度な依存（25以上のuse文）
単一ファイルで多数のモジュールに依存しており、責務が集中しすぎています。

## 特定された問題点

### 1. アーキテクチャレベルの問題
- **循環依存**: core ⇄ context の相互依存
- **層の侵害**: 下位層が上位層に依存するケース
- **モジュール境界の曖昧さ**: 責務が不明確

### 2. 実装レベルの問題
- **巨大なモジュール**: actor_context.rsが過度に複雑
- **再エクスポートの混乱**: dispatchがprocessの一部を再エクスポート
- **インターフェースの分散**: 共通のトレイトが複数箇所に散在

### 3. 保守性の問題
- **変更の波及範囲が大きい**: 一つの変更が多くのモジュールに影響
- **テストの困難さ**: 依存関係が複雑でモックが作りにくい
- **新規開発者の理解困難**: どこから読めばよいか不明確

## リファクタリング優先順位

### 優先度1: 循環依存の解消
1. core ⇄ context の循環依存を解消
2. 共通インターフェースを`traits`モジュールに抽出
3. 依存の方向を一方向に統一

### 優先度2: モジュール境界の明確化
1. 各モジュールの責務を定義
2. 公開APIを最小限に制限
3. 内部実装の詳細を隠蔽

### 優先度3: 複雑なモジュールの分割
1. actor_context.rsを機能ごとに分割
2. 責務を明確にした小さなコンポーネントに分解
3. 単一責任の原則を適用

## 次のステップ

1. このドキュメントをベースに、具体的なリファクタリング計画を策定
2. 循環依存を解消するための設計案を作成
3. テストが全て通ることを確認してから実装開始

## 付録: 依存関係グラフ

詳細な依存関係グラフは、以下のツールで生成可能：
```bash
cargo depgraph --workspace-only
```