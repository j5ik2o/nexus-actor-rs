# tracing/tokio-console 計測レポート (ActorContext)

## 計測概要
- 日付: 2025-09-26 (JST)
- ブランチ: refactor-0925
- 計測対象: modules/actor-core/examples/actor-context-lock-tracing (ActorContextExtras ホットパス負荷シナリオ)

## セットアップ
- 使用コマンド: `RUSTFLAGS="--cfg tokio_unstable" RUST_LOG="info,nexus_actor::lock_wait=debug,actor_context_lock_tracing::summary=info" cargo run -p nexus-actor-core-rs --features tokio-console --example actor-context-lock-tracing`
- feature flags: `tokio-console`
- RUSTFLAGS: `--cfg tokio_unstable`

## 計測結果サマリ
- 観測されたホットスポット: `ActorContextExtras::kill_receive_timeout_timer (write)`, `ActorContextExtras::get_receive_timeout_timer (read)`, `ActorContextExtras::get_children (read)` など Timer/Stash 周辺のロックスコープ
- ロック待ち平均/最大:
  - kill_receive_timeout_timer (write): max 130.916 µs / avg 2.41 µs (samples 1,029)
  - get_receive_timeout_timer (read): max 55.792 µs / avg 1.93 µs (samples 10,256)
  - get_children (read): max 25.125 µs / avg 1.94 µs (samples 32,768)
- 備考: receive_timeout 系処理が write ロックを長く保持しており、watcher/children 参照パスも read ロックの待ち時間を発生させている。DelayQueue 置換とロック粒度分離の優先度が裏付けられた。

## 詳細ログ
```text
2025-09-26T12:52:59.563136Z INFO actor_context_lock_tracing::summary: component="ActorContextExtras" operation="kill_receive_timeout_timer" mode="write" samples=1029 max_wait_us=130.916 avg_wait_us=2.410553935860058
2025-09-26T12:52:59.563188Z INFO actor_context_lock_tracing::summary: component="ActorContextExtras" operation="get_receive_timeout_timer" mode="read" samples=10256 max_wait_us=55.792 avg_wait_us=1.9260267160686428
2025-09-26T12:52:59.563209Z INFO actor_context_lock_tracing::summary: component="ActorContextExtras" operation="get_children" mode="read" samples=32768 max_wait_us=25.125 avg_wait_us=1.935639678955078
2025-09-26T12:52:59.563228Z INFO actor_context_lock_tracing::summary: component="ActorContextExtras" operation="init_or_reset_receive_timeout_timer:set_timer" mode="write" samples=1024 max_wait_us=19.875 avg_wait_us=2.2273310546875
2025-09-26T12:52:59.563247Z INFO actor_context_lock_tracing::summary: component="ActorContextExtras" operation="get_watchers" mode="read" samples=24576 max_wait_us=18.041 avg_wait_us=1.9237991129557293
```

## 次のアクション
- [ ] ActorContextExtras の残ロック分離（PidSet は同期化済み）後に再計測して差分を取得
- [ ] DelayQueue 置換 PoC 完了後に receive_timeout 系メトリクスを更新
