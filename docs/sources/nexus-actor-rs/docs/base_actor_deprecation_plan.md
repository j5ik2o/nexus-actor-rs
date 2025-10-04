# BaseActor 廃止サマリ (2025-09-29 時点)

## 区分基準
- **完了済み**: BaseActor 系 API 撤去に伴う実装／ドキュメント対応で、main ブランチへ反映済みのもの。
- **残フォローアップ**: 今後も継続確認が必要な監視ポイント。

## 完了済み
- `modules/actor-core/src/actor/context/base_spawner.rs` では `ActorSpawnerExt` へ一本化済みで、旧 `BaseSpawnerExt` は `since = "1.2.0"` の `#[deprecated]` ブリッジのみ残存。互換レイヤーが実装上の入口になっていないことを確認。
- `modules/actor-core/examples/legacy/*` に旧サンプルを隔離し、`core/examples` 直下は Actor トレイト準拠の構成のみ。（`rg "BaseActor" core/examples` で呼び出しが legacy のみであることを確認）
- テスト群は `modules/actor-core/src/actor/context/base_spawner.rs` の `test_spawn_actor` 系が Actor トレイトベースで成功することを保証し、BaseActor 固有テストは削除済み。
- ドキュメント類（`CLAUDE.md`, `PROJECT_STATUS.md`, `docs/legacy_examples.md`）は Actor トレイト統一方針へ書き換え済み。`rg "BaseActor" docs` で残存参照はリリースノート／履歴説明のみであることを確認。

## 残フォローアップ
- 互換 API (`BaseSpawnerExt`) は当面残置する方針のため、公開 API からの呼び出し件数を `cargo clippy -- -W deprecated` で監視する。
- リリースノート (`docs/releases/2025-09-26-actor-trait-unification.md`) を更新し、新規ブレイキングチェンジが発生した際は当ページを再検証する。

