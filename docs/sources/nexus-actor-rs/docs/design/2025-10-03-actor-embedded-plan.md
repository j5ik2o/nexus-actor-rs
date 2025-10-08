# actor-embedded クレートメモ（更新版）

現状の `modules/actor-embedded` は `ImmediateSpawner` / `ImmediateTimer`、ローカルメールボックス実装など最小限の no_std コンポーネントを提供している。旧版メモで触れていた `EmbeddedRuntimeBuilder` や `CoreSpawner` などの新 API はまだ存在しないため、該当記述を撤去した。今後 Embassy 連携を進める場合は以下のタスクに集中する。

## MUST
- `EmbeddedRwLock` の API・実装を維持し、actor-core が要求する複数 read／単一 write の動作を継続検証する。
- `CoreScheduler` の再設計を進め、Embassy で `schedule_once`/`schedule_repeated` に相当する API を提供できるよう設計案と互換性評価をまとめる。

## SHOULD
- ロギング Hook を検討し、no_std 環境でも `tracing` などを注入できる仕組みを整える。
- Mutex/RwLock/Notify など embedded 向けユニットテストを拡充し、`cargo test -p nexus-utils-embedded-rs --no-default-features --features embassy` が継続的に成功することを確認する。

## COULD
- Embassy 実行環境でのベンチマークやサンプル (`examples/embedded_blink.rs` など) を整備し、将来的な `Spawn` 拡張の検証素材とする。
