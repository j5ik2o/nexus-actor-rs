# actor モジュール分割タスク (2025-10-01 時点)

## 区分基準
- **ステータス別**: `完了済み` は既に実施済みのタスク、`継続タスク` は今後着手すべきもの、`検証・ドキュメント` は実装完了後に行う確認作業。

## 完了済み（2025-10-01）
- Criterion ベンチマークとサンプル（`modules/actor-core/{benches,examples}`）を `modules/actor-std` へ移動し、actor-core の開発用アセットを標準実装側に集約。
- actor-std の Cargo 設定を更新し、ベンチ／サンプルで必要な `tokio`・`criterion`・`clap` などを dev-dependencies として追加。
- actor-core からベンチ定義と関連 dev-dependencies (`clap`, `governor`, `humantime`, `criterion`) を削除し、テスト用依存を `rstest` / `loom` のみに整理。
- `modules/actor-core` → `modules/actor-core` へのリネームを実施し、`nexus-actor-core-rs` のパスを更新。
- `modules/actor-std` クレートを新設し、現状は `nexus-actor-core-rs` の公開 API を再エクスポートする構成に仮置き。
- ルート `Cargo.toml` に `modules/actor-core` / `modules/actor-std` を追加し、`remote` / `cluster` を `nexus-actor-std-rs` へ付け替え。
- 依存洗い出しのために `rg "tokio" modules/actor-core -n` / `rg "std::" modules/actor-core -n` と `cargo tree -p nexus-actor-core-rs` を実行し、std 依存を棚卸し。

## 継続タスク（実装）
1. **機能分割**
   - `tokio`・`parking_lot`・`std::net` 等へ依存する実装を actor-std 側へ移設し、actor-core を `alloc` ベースへ縮減。
   - actor-core と actor-std の境界を整理し、Mailbox/Dispatcher などの抽象インターフェースを共有化。
2. **テスト／ベンチ移行**
   - `tokio::test` や Criterion ベンチマーク群を actor-std に移動し、actor-core 側は no_std で動作確認できる最小構成へ絞り込む。

## 検証・ドキュメント（優先度順）
1. `cargo check -p nexus-actor-core-rs --no-default-features --features alloc` を実行し、actor-core 単体で no_std ビルドが通るか確認する。
2. `cargo test --workspace` と `cargo bench -p nexus-actor-std-rs` を実行し、統合後も既存の挙動と性能測定が成立するか検証する。
3. `docs/` 配下の関連ドキュメント（例: `core_improvement_plan.md`, `mailbox_*` 系）を actor-core / actor-std の役割に合わせて更新し、変更点を MECE に整理する。
4. 変更内容をまとめたリリースノート草案（CHANGELOG もしくは docs/worknotes）を作成し、依存プロジェクトへの影響を明記する。
