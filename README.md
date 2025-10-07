# nexus-actor-rs

[![ci](https://github.com/j5ik2o/nexus-actor-rs/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/j5ik2o/nexus-actor-rs/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/nexus-actor-core-rs.svg)](https://crates.io/crates/nexus-actor-core-rs)
[![docs.rs](https://docs.rs/nexus-actor-core-rs/badge.svg)](https://docs.rs/nexus-actor-core-rs)
[![Renovate](https://img.shields.io/badge/renovate-enabled-brightgreen.svg)](https://renovatebot.com)
[![dependency status](https://deps.rs/repo/github/j5ik2o/nexus-actor-rs/status.svg)](https://deps.rs/repo/github/j5ik2o/nexus-actor-rs)
[![License](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![License](https://img.shields.io/badge/License-APACHE2.0-blue.svg)](https://opensource.org/licenses/apache-2-0)
[![](https://tokei.rs/b1/github/j5ik2o/nexus-actor-rs)](https://github.com/XAMPPRocky/tokei)

`nexus-actor-rs` embodies the essence of the Actor Model, cleverly combining "Nexus" (connection, linkage, or center) with "actor" and the Rust programming language suffix "rs". This name represents the core principles of our project for the following reasons:

- Connection and Interaction: Nexus symbolizes the central point where various elements connect, reflecting the communication and interaction concepts in the Actor Model. The "actor" part emphasizes the active nature of these connections.
- Distribution and Integration: It illustrates how the distributed elements (Actors) of a system are interconnected, forming a cohesive whole. The "rs" suffix directly indicates the project's implementation in Rust, known for its focus on safe concurrency.
- Flexibility and Resilience: Nexus suggests a dynamically formed connection point, implying the system's flexibility and resilience. The straightforward structure of "actor-rs" reflects this clarity in its very name.
- Abstract yet Tangible Concept: While Nexus represents the essential structure and behavior of the system, "actor-rs" grounds it in the concrete implementation of actors in Rust.
- Multifaceted Meaning: nexus-actor-rs comprehensively expresses the diverse aspects of the Actor Model—computation, communication, structure, and interaction—while also clearly indicating the project's technical foundation.

`nexus-actor-rs` integrates the core characteristics of the Actor Model—distribution, interaction, modularity, and resilience—into a single, memorable concept. It represents not just the essence of the system's structure and behavior, but also embodies the practical spirit of the Rust community.
This name serves as a nexus itself, connecting the theoretical underpinnings of the Actor Model with the practical implementation in Rust, all while clearly communicating its purpose and technology stack.

---

## Installation

To add `nexus-actor-core-rs` to your project, follow these steps:

1. Open your `Cargo.toml` file.

2. Add the following line to the `[dependencies]` section:

```toml
nexus-actor-core-rs = "${version}"
```

Specify the version number in ${version}, for example 0.0.1.

3. If you want to use the latest version, you can check it by running:

```shell
cargo search nexus-actor-core-rs
```

4. To update the dependencies, run the following command in your project's root directory:

```shell
cargo update
```

Now `nexus-actor-rs` is installed and ready to use in your project.

Note: As versions may be updated regularly, it's recommended to check for the latest version.

## Developer Resources

- [Typed Context / PID ガイドライン](docs/sources/nexus-actor-rs/docs/typed_context_guidelines.md): ライフタイム指向設計に合わせた `ContextHandle` / `ActorContext` の扱い方や、弱参照化ポリシーをまとめています。開発時はこちらを参照してください。
- [Dispatcher Runtime ポリシー](docs/sources/nexus-actor-rs/docs/dispatcher_runtime_policy.md): `SingleWorkerDispatcher` など Runtime を内包する dispatcher 実装の shutdown 手順と運用上の注意点を整理しています。
- [ベンチマークダッシュボード](https://j5ik2o.github.io/nexus-actor-rs/bench_dashboard.html): GitHub Pages 上で週次ベンチのトレンドを確認できます。履歴 CSV は `benchmarks/history/bench_history.csv` に公開されています。
- [ActorContext ロック計測レポート](docs/sources/nexus-actor-rs/docs/benchmarks/tracing_actor_context.md): tokio-console/tracing を用いた ActorContext 周辺のロック待ち分析とホットスポットのまとめです。
- [ReceiveTimeout DelayQueue PoC](docs/sources/nexus-actor-rs/docs/benchmarks/receive_timeout_delayqueue.md): DelayQueue を用いた receive timeout の再アーム性能ベースラインと PoC コードの解説です。
- [Actor トレイト統一リリースノート](docs/sources/nexus-actor-rs/docs/releases/2025-09-26-actor-trait-unification.md): BaseActor 廃止と `ActorSpawnerExt` 追加に関する移行ガイドです。
- [レガシーサンプル一覧](docs/sources/nexus-actor-rs/docs/legacy_examples.md): 互換性維持のため `modules/actor-core/examples/legacy/` に隔離した旧サンプルの一覧です。
- [Tokio 向けディスパッチループ例](docs/worknotes/2025-10-07-tokio-dispatcher.md): `run_until`/`run_forever` を活用した常駐タスクの起動方法と実行例。
- [Embassy ブリッジメモ](docs/worknotes/2025-10-07-embassy-dispatcher.md): `spawn_embassy_dispatcher` で Embassy executor へ統合する手順。
- `modules/actor-embedded/examples/embassy_run_forever.rs`: Embassy executor 上で `TypedActorSystem` を常駐させる最小サンプル。
- [dispatch_all 非推奨ガイド](docs/design/2025-10-07-dispatch-transition.md): `dispatch_next` ベースへの移行ステップと TODO を整理しています。

## 最近の API 更新 (2025-10-07)

- `QueueMailbox::recv` は `Result<M, QueueError<M>>` を返すようになりました。`Ok` 以外が返った場合は閉鎖・切断を意味するため、`match` で明示的に停止処理を入れてください。
- `PriorityScheduler::dispatch_all` は非推奨です。`dispatch_next` / `run_until` / `run_forever` を利用してください。詳細は「dispatch_all 非推奨ガイド」を参照。
