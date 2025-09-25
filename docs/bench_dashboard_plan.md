# ベンチマークダッシュボード案（週次トレンド可視化）

## 目的
- `reentrancy` / `context_borrow` ベンチの結果を週次で収集し、性能トレンドを長期的に追跡できるようにする。
- 平均時間・標準偏差・サンプル数の推移を視覚化し、性能回帰の兆候を早期検知する。

## 収集フロー案
1. **スケジュール実行**
   - GitHub Actions で `schedule: cron`（毎週月曜 03:00 UTC など）を設定したベンチ専用ワークフローを追加。
   - 既存の `scripts/check_reentrancy_bench.sh` / `scripts/check_context_borrow_bench.sh` を `RUN_BENCH=1` の状態で実行し、Criterion の JSON 出力を生成。

2. **メトリクス整形**
   - ベンチ後に `jq` / `python` で `mean`, `median`, `std_dev`, `confidence_interval` を抽出し、CSV (append) 形式で `benchmarks/history/<bench_name>.csv` を更新する。
   - 更新済み CSV は `actions/upload-artifact` で保存し、`gh-pages` もしくは `bench-history` ブランチへ push する。

3. **データ保存戦略**
   - 簡易案: GitHub Pages + `benchmarks/history/*.csv` をそのまま公開。
   - 拡張案: S3/BigQuery など外部ストレージにアップロードしてもよいが、初期段階は GitHub 上で完結させる。

## 可視化案
- **CSV + Observable Plot**: GitHub Pages にホストした HTML (静的) で Observable Plot / Chart.js を利用し、CSV から折れ線グラフを描画。
- **Grafana Cloud**: データを Pushgateway 経由で送信し、Grafana 上でダッシュボードを構築（運用コスト増のため後回し可）。
- **Google Data Studio**: GitHub Actions で Google Sheets に追記し、Data Studio で可視化。

初期段階では GitHub Pages + Chart.js 構成が最小コスト。

## タスク概要
1. `bench-weekly.yml` ワークフローを追加し、`cron` スケジュールでベンチ実行。
2. ベンチ後に `scripts/export_bench_metrics.py`（新規）で JSON → CSV 変換。
3. CSV を `benchmarks/history/*.csv` に保存し、`actions/upload-artifact` および `peaceiris/actions-gh-pages` 等で `gh-pages` へ公開。
4. `docs/bench_dashboard.html` を用意し、Chart.js などで CSV を読み込んでグラフ化。
5. README からダッシュボードへのリンクを追加。

## 期待するアウトプット
- 週次実行されたベンチ履歴が CSV として蓄積。
- GitHub Pages 上のダッシュボードから平均時間や標準偏差の推移を確認できる。
- CI が閾値超過を検知した場合でも、トレンドグラフから経時的な傾向を評価可能。

## 補足
- CSV 格納ディレクトリ例: `benchmarks/history/reentrancy.csv`, `benchmarks/history/context_borrow.csv`
- CSV フォーマット（想定）: `timestamp,bench,mean_ms,median_ms,stddev_ms,samples`
- 拡張時には PR ごとに短期ベンチ（遅延測定）と週次ベンチ（トレンド）でワークフローを分離する。
