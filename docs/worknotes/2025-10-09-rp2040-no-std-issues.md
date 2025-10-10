# RP2040 (thumbv6m) での `no_std` ビルド課題メモ

## 背景
- `modules/actor-embedded/examples/rp2040_behaviors_greeter.rs` を `thumbv6m-none-eabi` で動かすためにビルドを試行。
- `nexus-actor-core-rs`／`nexus-utils-*` は `alloc::sync::Arc` や `futures::task::AtomicWaker` など CAS 前提の仕組みを多用。
- RP2040 (Cortex-M0+) はハードウェア CAS を持たず、`target_has_atomic = "ptr"` が偽になるため `alloc::sync` が提供されない。

## 発見した問題
- `alloc::sync::Arc` が未定義のため、`actor-core` 全体で大量の unresolved import が発生。
- `futures::task::AtomicWaker` も `target_has_atomic = "ptr"` 前提でしか公開されず、ASK パターンのコードがコンパイル不能。
- `spin` 依存は `portable_atomic` オプションで一部緩和できるが、`Arc` 依存の除去までは至らない。

## 対応済みの改善
- `nexus-utils-core-rs` の `Flag`／`DeadlineTimerKeyAllocator` などは `target_has_atomic` を参照し、`Cell + critical_section::with` で動くよう修正。
- `spin` は `portable_atomic` ＋ `spin_mutex` を有効化し、CAS をエミュレート可能な構成に変更。
- 上記により、最小限のユーティリティ層では RP2040 でもビルド可能なパスを確保。

## なお未解決の課題
- `nexus-actor-core-rs` の主要 API (`ActorSystem`/`ActorRef`/`Behavior` など) で `Arc` 前提の設計が残っており、`Rc` ベースへの抽象化が未実装。
- `AtomicWaker` に依存する ASK 実装も CAS 必須のため、RP2040 向けには代替実装 or feature 切り替えが必要。
- これらを解決するには Shared 抽象の再設計 or RP2040 専用の軽量ランタイム分岐が必要で、大規模改修になる。

## 回避策メモ
- RP2040 ではなく RP2350 (Cortex-M33, `thumbv8m.main-none-eabihf`) 以上をターゲットにすれば CAS が利用可能で、現状設計でもビルド可能な見込み。
- RP2040 サポートを進める場合は、`Arc` を排除できる Shared 抽象・`AtomicWaker` 代替を設計してから着手する。

## 次のアクション候補
1. RP2350 向けに LED ブリンクサンプルを整備し、当面の組み込み向け実行環境を確保。
2. RP2040 対応を進めるなら、Shared 抽象／メールボックス周辺の `Arc` 依存を段階的に `Rc`＋`critical_section` に移行する設計検討を開始。
3. 進捗が出次第、このメモを更新し、README などに現状の制約を追記する。
