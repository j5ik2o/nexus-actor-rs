# Mailbox ベンチマーク比較手順 (2025-09-29 時点)

## 区分基準
- **目的**: 手動ベースラインを取得する状況。
- **手順**: コミット間比較を行う具体的ステップ。
- **補足**: 日次ジョブとの整合や注意事項。

## 目的
- Nightly ベンチ (`.github/workflows/mailbox-sync-nightly.yml`) で異常を検知した際のローカル再現手順として利用する。
- 同期化前 (`0ae465fc312eceb863297ed5efd549909648fa89`) と同期化後の性能差を再確認したい場合に実施する。

## 手順
1. `git checkout 0ae465fc312eceb863297ed5efd549909648fa89`（同期化前）を取得し、`modules/actor-core/benches/mailbox_throughput.rs` を現行ブランチへコピーする。
2. `cargo bench --bench mailbox_throughput -- --samples 100` などで計測し、`target/criterion/mailbox_throughput/.../new/benchmark.csv` を保存。
3. 現行コミットへ戻り再度 `cargo bench --bench mailbox_throughput` を実行し、`criterion diff` または CSV 比較で差分を確認。
4. 結果は `docs/mailbox_sync_transition_plan.md` の「Nightly ベンチ」節へ追記し、今後の基準に反映する。

## 補足
- `cargo bench -- --baseline sync` で Nightly が保存したベースラインと同条件比較が可能。
- 測定環境の揺らぎを抑えるため、CPU スケーリングを固定し `RUSTFLAGS="-C target-cpu=native"` を揃える。
