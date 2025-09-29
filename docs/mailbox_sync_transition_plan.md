# DefaultMailbox 同期化ロードマップ (2025-09-29)

## ゴール
1. DefaultMailbox を同期キュー化し、ホットパスから不要な `await` を排除する。
2. Remote/cluster/integration の全経路で同期化 DefaultMailbox を適用し、互換レイヤは撤廃する。
3. Nightly ベンチ（mailbox_throughput）の結果を継続的に確認し、劣化があれば即対応する。

## 現況 (2025-09-29)
- DefaultMailbox 本体とハンドル層（SyncQueue）リファクタは完了済み。
- Remote: `EndpointWriterMailbox` を `parking_lot::Mutex` + SyncQueue API で再実装済み。
- Cluster: 専用 mailbox はなく、DefaultMailbox 同期化版をそのまま利用している。
- Integration: `remote/src/tests.rs` で同期化 mailbox を利用。互換ブリッジ経路は存在しない。
- Nightly: `.github/workflows/mailbox-sync-nightly.yml` で `cargo bench --bench mailbox_throughput -- --save-baseline sync` を毎日実行。

## TODO
- **MUST-2**: Virtual Actor 実装が追加されたら DefaultMailbox を直適用し、残る互換アダプタを撤廃する。
- **MUST-3**: Nightly ベンチ結果を定期確認（追加の S3/Slack/Grafana 連携は不要）。
- **MUST-P**: Legacy ユーティリティ／メトリクス最適化は必要に応じて後追い実施。

