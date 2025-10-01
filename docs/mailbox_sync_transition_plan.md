# DefaultMailbox 同期化ロードマップ (2025-09-29 時点)

## 区分基準
- **ゴール**: ロードマップ全体の到達点。
- **現況**: 2025-09-29 時点で達成済みの事項。
- **TODO**: 残課題。

## ゴール
1. DefaultMailbox を同期キュー化し、ホットパスから不要な `await` を排除する。
2. Remote / Cluster / Integration の全経路で同期化 DefaultMailbox を適用し、互換レイヤを撤廃する。
3. Nightly ベンチ（`mailbox_throughput`）で継続監視し、性能回帰を即検知する。

## 現況
- DefaultMailbox 本体とハンドル層（Queue ベース）リファクタは完了。`modules/actor-core/src/actor/dispatch/mailbox/default_mailbox.rs` の `post_*`/`poll_*` は同期化済み。
- Remote: `EndpointWriterMailbox` が `parking_lot::Mutex` + Queue API を採用。
- Cluster: Virtual Actor 経路を含め専用 mailbox は不要となり、DefaultMailbox を直接利用。
- Integration: `remote/src/tests.rs` で同期化 mailbox が通過する経路をカバーし、互換ブリッジは存在しない。
- Nightly: `.github/workflows/mailbox-sync-nightly.yml` で `cargo bench --bench mailbox_throughput -- --save-baseline sync` を毎日実行。

## TODO
- Virtual Actor 実装で新規 mailbox を導入する必要が生じた場合も同期化構造を流用し、互換アダプタを増やさない。
- Nightly ベンチ結果を定期確認し、閾値逸脱時のレスポンス（Issue 起票/Slack 通知）を自動化する。
- Legacy メトリクスの整理（旧 API の削除）は後追いで検討。

