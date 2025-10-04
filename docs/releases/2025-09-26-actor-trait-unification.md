# 2025-09-26 Actor トレイト統一アップデート

## 区分基準
- **ハイライト**: リリース内容の概要。
- **マイグレーション**: 利用者が実施する対応。
- **既知の非互換/今後**: リスクとフォローアップ。


## ハイライト
- BaseActor / BaseContext / MigrationHelpers / ContextAdapter をコードベースから削除し、`Actor` トレイトのみに統一しました。
- `RootContext` には新たに `ActorSpawnerExt` を追加し、`spawn_actor` / `spawn_actor_named` で `Props::from_async_actor_producer` を直接利用できるようになりました。
- 旧 API `spawn_base_actor*` は非推奨ブリッジとして残存しますが、内部的に新 API を呼び出します。新規コードでは `ActorSpawnerExt` を使用してください。
- BaseActor 依存サンプルを `modules/actor-core/examples/legacy/` へ移動し、互換目的のみに隔離しました。

## マイグレーションガイド
1. `RootContext` 拡張として `ActorSpawnerExt` を `use nexus_actor_core_rs::actor::context::ActorSpawnerExt;` でインポートし、`spawn_actor` / `spawn_actor_named` へ置き換えてください。
2. BaseActor トレイトや `MigrationHelpers` を利用していた場合は、`Props::from_async_actor_producer` 経由で `Actor` トレイト実装を生成するよう変更してください。
3. 旧サンプルが必要な場合は `modules/actor-core/examples/legacy/` を参照してください。新しいサンプルは `actor-advanced-migration` 等を参照することを推奨します。

## 既知の非互換
- BaseActor 系 API は crate から完全に削除されています。コンパイル時に未解決シンボルが発生した場合は、上記マイグレーションガイドに従ってください。
- `ActorSpawnerExt` への移行によって API 名称が変更されました。`spawn_base_actor*` を呼び出すコードは引き続きコンパイル可能ですが、非推奨警告が表示されます。

## 今後の予定
- `spawn_base_actor*` ブリッジの段階的削除スケジュールを策定し、次回リリースで非推奨アナウンスを強化します。
- `legacy/` ディレクトリを整理し、必要最小限のサンプルのみを残す予定です。
