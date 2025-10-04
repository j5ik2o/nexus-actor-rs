# レガシーサンプル棚卸し (2025-09-29 時点)

## 区分基準
- **現存レガシー**: `modules/actor-core/examples/legacy` に残した BaseActor 時代のスナップショット。
- **扱い方針**: 今後の維持／削除に関するルール。

## 現存レガシー
- `modules/actor-core/examples/legacy/actor-base-traits`
- `modules/actor-core/examples/legacy/actor-base-with-props`
- `modules/actor-core/examples/legacy/actor-dual-support`
- `modules/actor-core/examples/legacy/actor-migration`

## 扱い方針
- 教材用途のため当面残置するが、新規コードは Actor トレイト版のみを使用する。必要時は `cargo run --example <name>` で挙動確認。
- BaseActor 系 API を完全削除するタイミングで、各ディレクトリをアーカイブまたは `docs/releases` に引用のみ残す方針を検討する。

