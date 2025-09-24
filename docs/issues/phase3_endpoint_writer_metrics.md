# Issue: Phase 3 EndpointWriter 改修・メトリクス整備・ドキュメント更新

## 背景
- Phase 1.5 で EndpointManager／EndpointState を拡張し、backpressure と再接続の骨子を整備した。
- しかし EndpointWriter 側は依然として単純な接続処理のみで、ストリーム切断時の再接続処理や送信メトリクスの露出が不足している。
- 運用面での観測性向上とドキュメント整備が求められており、protoactor-go 実装とのパリティを取る必要がある。

## 要件レベル表記
- **MUST**: Phase 3 内で必ず実施する項目。
- **SHOULD**: 可能であれば実施したい項目（遅延があっても致命的でない）。
- **MAY**: 任意対応（後続フェーズでも可）。
- **MUST NOT**: Phase 3 では実施しない項目。

## ゴール
- **MUST** EndpointWriter に指数バックオフ付き再接続処理を実装し、切断時もメッセージ配送が回復できるようにする。
- **MUST** 送信成功・失敗・再接続試行などのメトリクスを収集し、EventStream やメトリクスエクスポートで観測可能にする。
- **MUST** ドキュメント／サンプルを更新し、利用者が設定・監視手順を理解できる状態にする。

## スコープ
- **MUST** `remote/src/endpoint_writer.rs` の再接続ロジック実装（指数バックオフ、DeadLetter 連携の調整）。
- **MUST** `EndpointManager` 統計との連携を踏まえたメトリクス発火（`metrics` crate もしくは標準 EventStream の活用）。
- **MUST** gRPC 切断→再接続の結合テスト追加、およびメトリクス検証用テスト。
- **MUST** `docs/` や `remote/examples/` の更新（再接続シナリオ、メトリクス取得、BlockList API との整合）。

## 非スコープ
- **MUST NOT** TLS 導入（Phase 3 では扱わない）。
- **MUST NOT** BlockList API／ConfigOption の追加改修（Phase 2 で実施予定）。

## 現状整理
- EndpointWriter は初回接続成功後の切断に対する再接続処理を持たず、EndpointManager の `schedule_reconnect` との整合も不十分。
- DeadLetter は EndpointWriterMailbox で検知するが、メトリクスで可視化できていない。
- ドキュメント／サンプルに再接続手順やメトリクス取得方法が未記載。

## 参考実装
- protoactor-go `remote/endpoint_writer.go` の `ensureConnected`、`handleConnectionFailure` など。
- Phase 1.5 で整備済みの `EndpointState`、`EndpointManager::await_reconnect`、backpressure 統計。

## 設計案
### A. EndpointWriter 再接続ロジック
- **MUST** トニックのストリームが `Status` エラーを返したら `EndpointManager::schedule_reconnect` を呼び出し、指数バックオフで再接続を試みる。
- **MUST** 再接続完了までの間は追加 Deliver を DeadLetter にフォールバックし、統計に反映する。
- **MUST** 成功／失敗時に `EndpointThrottledEvent(level=Normal)` や `DeadLetterEvent` を送信する。

### B. メトリクス整備
- **MUST** 送信成功数・失敗数・再接続試行回数を `EndpointStatistics` に counters として追加し、`statistics_snapshot` から参照できるようにする（metrics crate 導入が難しい場合は EventStream ログでフォールバック）。
- **MUST** 再接続試行時に EventStream へ `EndpointReconnectEvent`（仮称）を発火し、外部で監視できるようにする。
- **SHOULD** EndpointManager 統計（queue_size, dead_letters）と整合する形で Prometheus 互換メトリクスを設計する（外部エクスポートは Phase 3.5 以降に継続検討）。

### C. ドキュメント・サンプル
- **MUST** 再接続を含むサンプルコードを `remote/examples/` に追加し、`cargo run --example reconnect` で確認可能にする。
- **MUST** `docs/` に「再接続ポリシーの設定」「メトリクス観測方法」などを追記し、LAN 内クラスタ運用向けの手順を整備する。
- **MAY** metrics ダッシュボード例（Prometheus/Grafana）を紹介する。

## テスト計画
- **MUST** ユニットテストで EndpointWriter の再接続処理がバックオフ間隔に従って動作し、メトリクスが更新されることを確認する。
- **MUST** 結合テストとして `remote_reconnect_after_server_restart` を拡張し、連続切断シナリオでも再接続できることを検証する。
- **SHOULD** モック exporter などを用いて送信成功／失敗メトリクスがカウントされることをテストする。

## ドキュメント更新
- **MUST** `docs/issues/phase1_5_endpoint_stream.md` の Follow-up を Phase 3 要件に合わせて整理する。
- **MUST** `docs/remote機能改善計画.md` に本 Issue のタスクを追加し、完了後の進捗を反映する。
- **MAY** README など利用者向けドキュメントに再接続・メトリクスのクイックスタートを追加する。

## タイムライン（目安）
| タスク | 期間目安 |
|--------|----------|
| 設計ドラフト作成 (**MUST**) | 1日 |
| EndpointWriter 再接続実装 (**MUST**) | 2日 |
| メトリクス導入・テスト (**MUST**) | 2日 |
| ドキュメント／サンプル更新 (**MUST**) | 1日 |

不明点があればコメントでフィードバックをお願いします。
