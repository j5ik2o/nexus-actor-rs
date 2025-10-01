# actor モジュール分割タスク (2025-10-01 時点)

## 区分基準
- **ステータス別**: `完了済み` は既に実施済みのタスク、`継続タスク` は今後着手すべきもの、`検証・ドキュメント` は実装完了後に行う確認作業。

## 完了済み（2025-10-01）
- 既存 `modules/actor-core` を `modules/actor-std` へ移設し、Tokio/std 依存の本体・ベンチ・サンプルを `nexus-actor-std-rs` に集約。
- `modules/actor-core` を `#![no_std]` + `alloc` ベースの最小スケルトンに再構築し、コア API を再編する準備を完了。
- `actor-std` の Cargo 設定を整理し、従来の依存関係（Tokio, opentelemetry など）を引き継ぎつつ `nexus-actor-core-rs` に依存させる形へ更新。
- `actor-core` に `actor::core_types::message` モジュールを追加し、`Message`/`ReceiveTimeout`/`TerminateReason` などの no_std 対応な基盤型を定義。`actor-std` は同モジュールを再エクスポートし、標準実装向けヘッダー拡張などを追加する構成に変更。
- ランタイム抽象（AsyncMutex/RwLock/Notify/Timer）を actor-core に追加し、actor-std で Tokio 実装を提供。スタッシュ操作など `tokio::sync` 直参照部分をアダプタ越しに置換。
- `cargo test --workspace` が新構成でも成功することを確認。

## 継続タスク（実装）
1. **抽象レイヤ設計**
   - ロック／タイマー／チャネル等の Tokio 依存箇所を抽象化し、actor-core では trait のみ保持、actor-std が Tokio 実装を提供する構造へ段階的に移行する。
   - 抽出候補モジュール（例: mailbox, process, supervisor）の依存を洗い出し、優先度順に分割プランを立案。
2. **依存モジュールの移植**
   - `alloc` だけで動くコンポーネント（PID, middleware, Serialized message handles など）を actor-core に移し、必要に応じて `alloc::` 系型や `hashbrown` への置き換えを行う。
   - 移植後は actor-std 側で `pub use` を通じて互換性を維持しつつ、Tokio 依存のエクステンションのみを残す。

## 検証・ドキュメント（優先度順）
1. `cargo check -p nexus-actor-core-rs --no-default-features --features alloc` を追加し、actor-core 単体の no_std ビルドが通るタイミングを随時確認。
2. `cargo test --workspace` と `cargo bench -p nexus-actor-std-rs` を継続実行し、分離作業による回帰を監視。
3. `docs/` 配下（`core_improvement_plan.md` など）を actor-core / actor-std の役割に合わせて更新し、作業完了段階で MECE に整理。
4. 変更内容を整理したリリースノート草案を作成し、依存プロジェクトへの影響範囲（新たな `nexus-actor-core-rs` の位置付け）を明記する。
