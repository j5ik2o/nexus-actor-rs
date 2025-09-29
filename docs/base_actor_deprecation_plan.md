# BaseActor 廃止ロードマップ (2025-09-26)

## 目的
BaseActor / BaseContext 系 API を段階的に削除し、従来の Actor トレイト実装へ統一する。

## 現状整理
- **ドキュメント**: PROJECT_STATUS.md, CLAUDE.md で BaseActor 推奨記述あり。
- **examples**: actor-base-* / actor-migration / actor-dual-support などが BaseActor を前提に構成。
- **core 実装**:
  - 2025-09-26 時点で base_spawner.rs 以外の BaseActor 系補助を撤去済み。
  - 削除済み: context_base.rs, context_ext.rs, migration.rs, actor_behavior_base.rs, actor_handle_base.rs, adapters.rs (ContextAdapter/ActorBridge)。
  - **テスト**: BaseActor 依存テストは Actor ベースへ移行済み。

## 段階プラン
1. **ドキュメント更新**
   - PROJECT_STATUS.md と CLAUDE.md から BaseActor 推奨記述を削除/置換。
   - 外部向けに Actor トレイト統一方針を明記。
2. **examples の刷新**
   - BaseActor 依存サンプルを Actor トレイト版へ移行。不要なサンプルは削除か legacy として隔離。
3. **Core API の置換**
   - base_spawner.rs, context_ext.rs, actor_behavior_base.rs, actor_handle_base.rs を Actor ベース実装へ移行。
   - BaseActor 向けヘルパ関数・トレイトを削除。
4. **Migration ヘルパ整理**
   - MigrationHelpers と関連 API を段階的に無効化し、利用箇所を Actor API へ書き換え。
   - ContextAdapter, ActorBridge を削除。
5. **テスト/ビルド調整**
   - BaseActor 依存テストを Actor 版へ差し替え、CI を通す。
6. **最終クリーンアップ**
   - context_base.rs の BaseActor 系トレイトを完全削除。
   - 依存していたモジュール/テストを全て整理し、dead code がないことを確認。

## 次のアクション
- [x] PROJECT_STATUS.md / CLAUDE.md の内容更新
- [x] actor-base-traits を Actor トレイト版へ書き換え
- [x] actor-base-with-props を Actor トレイト版へ書き換え
- [x] actor-migration / actor-dual-support を Actor トレイト版へ書き換え
- [x] actor-advanced-migration を Actor トレイト版へ書き換え、GetStats 応答を再実装 (2025-09-26)
- [x] base_spawner.rs で BaseActor 依存を排除し、Actor トレイト版のラッパーへ置換 (2025-09-26)
- [x] context_ext.rs / actor_behavior_base.rs / actor_handle_base.rs を削除し、BaseActor ブリッジ API を整理 (2025-09-26)
- [ ] ドキュメント/設計メモから BaseActor 参照を洗い出し、最新の Actor 方針へ更新

## 整理メモ (2025-09-26 更新)
- base_spawner.rs: Actor トレイト用 `Props::from_async_actor_producer` を用いたクロージャ生成へ完全移行。
- BaseActor 系ユーティリティ (ContextAdapter, MigrationHelpers など) を削除し、従来の Actor トレイト API へ一本化。
- 既存 API (`spawn_base_actor*`) は互換性維持のため非推奨ブリッジとして残存するが、`ActorSpawnerExt` への移行を推奨。
- legacy 例 (`core/examples/legacy/*`) として旧ファイルを隔離し、新規利用を防止。

## 次の検討事項
- ドキュメント (CLAUDE.md, PROJECT_STATUS.md, typed_context_guidelines.md など) の BaseActor 記述を Actor 統一方針へ書き換える。
- `spawn_base_actor*` API の名称再検討および段階的非推奨アナウンス。
- BaseActor 廃止に伴う公開 API 変更点をリリースノート・CHANGELOG に整理。
