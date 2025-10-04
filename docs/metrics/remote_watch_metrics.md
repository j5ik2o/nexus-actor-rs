# Remote Watch Metrics

## 目次構成（分類基準: メトリクス種類別）
- 1. カウンタ系メトリクス
- 2. ゲージ系メトリクス
- 3. 活用シナリオ
- 4. アラート設計

## 1. カウンタ系メトリクス（分類基準: 監視イベント種別）
- `nexus_actor_remote_watch_event_count`
  - ラベル: `remote.endpoint`, `remote.watcher`, `remote.watch_action`
  - 意味: watch/unwatch/terminate の発生回数を集計。アクションごとの傾向を把握できる。
  - 可視化例: スタックド折れ線（action 別）、累積値のヒートマップ。

## 2. ゲージ系メトリクス（分類基準: ウォッチャ単位）
- `nexus_actor_remote_watchers`
  - ラベル: `remote.endpoint`, `remote.watcher`
  - 意味: ウォッチャが現在保持する Watchee 数。異常な監視リークを検知できる。
  - 可視化例: 上位 N ウォッチャの Watchee 数バーチャート、ウォッチャ×時間ヒートマップ。

## 3. 活用シナリオ（分類基準: 運用イベント）
- **監視リーク検知**
  - 指標: `remote.watch_action="watch"` の急増と `nexus_actor_remote_watchers` の高止まり。
  - アクション: 該当ウォッチャのアクター稼働状況調査、解除漏れの修正。
- **リモートエンドポイント障害**
  - 指標: `remote.watch_action="terminate"` のスパイクと EndpointTerminatedEvent の同時発生。
  - アクション: 対象エンドポイントへの疎通確認、再接続ポリシーの調整。
- **監視構成変更の効果測定**
  - 指標: 設定変更後の `remote.watch_action="watch"` / `"unwatch"` バランス。
  - アクション: 監視対象追加・削除手順の妥当性確認。

## 4. アラート設計（分類基準: 閾値種類）
- **ウォッチャ単位の上限閾値**
  - 条件: `nexus_actor_remote_watchers` が事前に定義した上限を継続的に超過。
  - 通知: Slack / PagerDuty。付随情報として `remote.watcher` と `remote.endpoint` を出力。
- **Terminate イベント急増**
  - 条件: `sum(rate(nexus_actor_remote_watch_event_count{remote.watch_action="terminate"}[5m]))` が過去７日移動平均の 3× を超過。
  - 通知: システム回復チーム。関連する Endpoint の接続統計（backpressure, reconnect count）をダッシュボードでリンク。
- **Unwatch 未達**
  - 条件: `sum(rate(nexus_actor_remote_watch_event_count{remote.watch_action="unwatch"}[15m])) < sum(rate(nexus_actor_remote_watch_event_count{remote.watch_action="watch"}[15m])) * 0.5`。
  - 通知: 運用担当へ監視解除フローの点検を促す。
