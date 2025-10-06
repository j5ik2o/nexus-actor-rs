# ベンチマークダッシュボード運用 (2025-09-29 時点)

## 区分基準
- **運用フロー**: 既に自動化されている収集・可視化手順。
- **改善候補**: 現状の欠落や次に手を付けるべき拡張案。
- **参照リソース**: 実装や設定ファイルの所在。

## 運用フロー
- `.github/workflows/bench-weekly.yml` で毎週月曜 03:00 (UTC) に `cargo bench -p nexus-actor-core-rs --bench reentrancy` を実行し、Criterion 出力を取得。
- `scripts/export_bench_metrics.py` が `criterion` データをランナーの一時ディレクトリに集約し、workflow 内で `peaceiris/actions-gh-pages@v4` により `gh-pages` ブランチへ公開（リポジトリには成果物を保持しない）。
- `docs/bench_dashboard.html` は Chart.js で CSV をロードし、平均応答時間とサンプル数を折れ線グラフ化。GitHub Pages から閲覧可能。

## 改善候補
- `context_borrow` など追加ベンチの CSV 分割（列 or ファイル）とグラフ追加。Criterion 出力は既存スクリプトでパース可能なため、引数化を検討。
- ベンチ履歴のローカル確認を容易にするため、`scripts/export_bench_metrics.py --output <path>` で任意の CSV を生成し共有するオプションを評価。
- CI 側で閾値逸脱を検知した際に Issue を自動起票する GitHub Actions ステップ（`gh issue create`）を追加し、アラートを可視化。

## 参照リソース
- Workflow: `.github/workflows/bench-weekly.yml`
- 変換スクリプト: `scripts/export_bench_metrics.py`
- ダッシュボード: `docs/bench_dashboard.html`
- ベンチ本体: `modules/actor-core/benches/reentrancy.rs`
