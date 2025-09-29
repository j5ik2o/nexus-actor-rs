# Mailbox ベンチマーク比較手順 (2025-09-28)

## 目的
`DefaultMailbox` 同期化前後の性能差を定量的に把握するため、同一ベンチ (`mailbox_throughput`) を使った比較手順をまとめる。

## 前提
- 同期化前のコミット (`<BASE_COMMIT>`) が参照可能であること。
- 同期化後は `bench/benches/mailbox_throughput.rs` と `bench/Cargo.toml` のエントリが存在する。

## 手順
1. **同期化前コミットをチェックアウト**
   - 推奨基準: `0ae465fc312eceb863297ed5efd549909648fa89`（同期キュー導入前の状態）
   ```bash
   git checkout 0ae465fc312eceb863297ed5efd549909648fa89
   ```
2. **ベンチファイルを配置**（同期化前コミットには存在しないため）
   - 現在のブランチから以下をコピーする:
     - `bench/benches/mailbox_throughput.rs`
     - `bench/Cargo.toml` 内の `mailbox_throughput` エントリ
   - 作業後に `git status` で追加ファイルのみ反映されていることを確認（コミット不要）。
3. **依存ビルド & ベンチ実行**
   ```bash
   cargo bench --bench mailbox_throughput
   ```
   - 出力された `mailbox_process/unbounded_mpsc_100` と `..._1000` の time/thrpt を記録。
4. **現行コミット（同期化後）に戻る**
   ```bash
   git checkout refactor-0925
   cargo bench --bench mailbox_throughput
   ```
5. **比較・記録**
   - `docs/mailbox_sync_transition_plan.md` のベンチ結果節に前後比較を追記。
   - 測定条件（CPU/負荷/ビルド設定）を同一に保つ。
   - 必要に応じて raw 出力 (`target/criterion/.../new/benchmark.csv`) を保存する。

## 備考
- ベンチは `criterion` を利用しているため、実行毎に若干のブレが生じる。必要に応じて `--samples` `--measurement-time` などで安定化する。
- 長期的には CI ジョブに測定を組み込み、レポートを自動生成することも検討する。
