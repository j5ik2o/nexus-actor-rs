# ActorContext ロック計測ノート (2025-09-29 時点)

## 区分基準
- **計測状況**: 実施済みベンチと未実施項目。
- **実行手順**: 再現に必要なコマンドと設定。
- **フォローアップ**: 次回更新で実施する作業。

## 計測状況
- 2025-09-29 時点で `modules/actor-core/benches/actor_context_lock.rs` の最新ベンチ結果は未計測。mailbox/dispatcher 関連ベンチにリソースを集中していたため。
- 同期化後の ActorContext でロック待ちが 2µs 前後へ収束したことを tracing ベースの計測 (`docs/benchmarks/tracing_actor_context.md`) で確認済み。

## 実行手順
```bash
cargo bench -p nexus-actor-core-rs --bench actor_context_lock -- --sample-size 100
```
- 実行後、`target/criterion/actor_context_lock/*/new/benchmark.csv` を `docs/benchmarks/` に反映する。
- `RUSTFLAGS="-C target-cpu=native"` を設定し、CPU スケーリングを固定すると再現性が向上。

## フォローアップ
- 次のベンチ実行時に平均値／p95／最大値を採取し、本ドキュメントへ追記する。
- DelayQueue 置換後に再度計測し、`receive_timeout_delayqueue.md` の結果と比較する。
