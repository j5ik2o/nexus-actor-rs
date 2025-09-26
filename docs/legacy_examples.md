# レガシー例の整理 (2025-09-26)

## 目的
- BaseActor ベースから移行済みのサンプルを `legacy/` ディレクトリへ隔離し、新しい Actor トレイト例と区別する。

## 現状リスト
- `core/examples/legacy/actor-base-traits`
- `core/examples/legacy/actor-base-with-props`
- `core/examples/legacy/actor-dual-support`
- `core/examples/legacy/actor-migration`

## 今後の方針
- これら legacy 例を段階的に削除またはドキュメント用スナップショットへ移行。
- 新しい Actor トレイト例を `core/examples/` 直下に配置して維持する。
