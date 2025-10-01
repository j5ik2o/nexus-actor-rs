# actor-core / actor-std 分割リリースノート草案 (2025-10-01)

## 区分基準
- **機能再編**: コア API の所在や依存関係の変更点。
- **移行手順**: 既存プロジェクトが取るべき対応策。
- **検証状況**: 実施済みビルド・テスト・ベンチ結果。
- **影響範囲**: 下流プロジェクト・統合環境での注記。

## 機能再編
- `nexus-actor-core-rs` が `#![no_std] + alloc` 前提の核として、`CorePid`・`CoreMessageEnvelope`・`CoreProcessHandle` など純粋データ抽象を提供。
- `nexus-actor-std-rs` は Tokio 依存機能・ProtoBuf 変換・ベンチ類を集約し、core 抽象を再エクスポートする構成に統一。
- WatchRegistry／Endpoint 系監視機能と Mailbox 抽象の Tokio 実装が `actor-std` へ集約され、コア層からの実装漏れが解消。
- EndpointSupervisor と RemoteProcess が WatchRegistry を共有し、重複 watch/unwatch の送信を抑制しつつイベント／メトリクスとメッセージ送信を同期。
- Mailbox 周りは `CoreMailboxQueue` を trait object 化し、MPSC／Ring／Priority 向けアダプタ導入で core 抽象と std 実装の責務境界を明確化。

## 移行手順
- 依存プロジェクトは `nexus-actor-std-rs` から `Core*` 型を参照する場合、`nexus-actor-core-rs` を明示依存に追加し no_std 経路でも利用可能にする。
- Tokio 以外のランタイムをターゲットにする場合は、`CoreProcessHandle` と Mailbox トレイトを差し替えて独自 executor へ適合させる。
- 旧 `modules/actor-core` の std API へ直接アクセスしていたコードは、`actor-std` の再エクスポート経由に切り替える。

## 検証状況
- `cargo check -p nexus-actor-core-rs --no-default-features --features alloc` を実行し、no_std + alloc 構成のビルド成功を確認。
- `cargo test --workspace` を完走、既存ユニットテスト・ドキュメントテストが全て成功。
- `cargo bench -p nexus-actor-std-rs` を完走し、Mailbox / Context 系ベンチマークが最新抽象で正常動作することを確認。

## 影響範囲
- `nexus-remote-core-rs` や `nexus-cluster-core-rs` は `CorePid` を介した呼び出しへ切り替え済みであり、新規 API への追従は完了。
- 下位互換性は保持しない方針のため、カスタム Mailbox 実装は新しい `CoreMailbox*` トレイトに対応させる必要がある。
- CI パイプラインでは no_std ビルドチェック・workspace テスト・actor-std ベンチを定期ジョブへ昇格させることを推奨。
