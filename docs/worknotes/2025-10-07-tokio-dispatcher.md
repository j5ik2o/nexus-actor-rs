# Tokio ランタイムでのスケジューラ常駐例 (2025-10-07)

## 目的
`TypedActorSystem::run_until`/`run_forever` を利用して Tokio 上でディスパッチループを常駐させる
方法を整理する。

## 必要条件
- `nexus-actor-std-rs` を依存に追加 (`Cargo.toml`)
- Tokio を `rt-multi-thread` もしくは `rt-current-thread` で有効化

## 手順
1. `TypedActorSystem` を生成し、`root_context()` からアクターを起動する。
2. `run_until` あるいは `run_forever` を `tokio::spawn` で実行する。
3. 停止条件が必要な場合は `Arc<AtomicBool>` 等で制御する。

## サンプル
`modules/actor-std/examples/tokio_run_forever.rs`

```shell
cargo run -p nexus-actor-std-rs --example tokio_run_forever
```

サンプル内部では `run_until` をループ条件付きで実行し、`AtomicBool` を用いて停止する。実アプリ
では `run_forever` をそのままタスク化し、アプリケーション終了時にタスクをキャンセルする構成を想定。

## TODO
- 実アプリ向けの graceful shutdown シグナル例（`Notify` や `broadcast`）を追記する。
- `run_forever` を直接利用する multi-thread 版のサンプルを追加する。
