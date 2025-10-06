# 所有モデル抽象化による std / embedded 両対応計画 (2025-10-04)

## 方針概要
- `core` クレートは `no_std + alloc` を前提とし、OS / スレッド / `Arc` に依存しない純粋ロジック層を構築する。
- `std` モジュールはサーバ／Linux 環境を対象とし、`Arc`・`Send`・`Sync`・`tokio` 等でフル機能化する。
- `embedded` モジュールは MCU 向けとし、`Arc` に頼らず `Rc` または `StaticRef` を利用する所有モデルを採用する。
- RP2040 / RP2350 など MCU によって原子命令サポートが異なるため、feature で `embedded_rc`（`Rc` ベース）と `embedded_arc`（`Arc` ベース）を切り替えられるようにする（将来的に Embassy などの実装を差し替えやすくする）。
- コアの公開 API は共通とし、所有モデルだけをトレイトで切り替える設計にする。
- この方針を今後の最優先タスクとする。

## Shared 抽象
- `Shared<T>` トレイトを導入し、`Clone` + `Deref<Target = T>` を満たす共有所有権を一般化する。
- 既存の `Arc` に加え、`Rc` や `&'static T` を `Shared` 実装として扱うことで、環境ごとの所有モデルを切り替える。
- `Shared<T>` は必要に応じて `try_unwrap(self)` を提供し、内部の値を取り出せるようにする。
- 使用例:

```rust
pub trait Shared<T>: Clone + core::ops::Deref<Target = T> {
  fn try_unwrap(self) -> Result<T, Self> where T: Sized { Err(self) }
}
```

### 実装例
- std 環境:
  - `impl<T> Shared<T> for alloc::sync::Arc<T> {}`
- embedded (原子命令無し, 単核):
  - `impl<T> Shared<T> for alloc::rc::Rc<T> {}`
- embedded (static メモリ配置):
  - `struct StaticRef<T>(&'static T);` を用意し `Shared` 実装。

## フィーチャ構成案

```toml
[features]
default = ["alloc", "embedded_rc"]
alloc = []
embedded_rc = []      # RP2040 等、シングルコア / 非アトミック環境
embedded_arc = ["embassy-sync", "critical-section"]     # RP2350 等、atomic 命令対応 MCU 向け（Arc + Embassy 同期）
embedded_static = []  # `StaticRef` による完全 static 構成（準備中）
```

- std / embedded で共有するコードは `Shared` を通じて抽象化。
- RP2350 のような atomic 対応 MCU では `Arc` をそのまま利用可能。

## Mailbox / Scheduler 抽象
- `Mailbox` や `Spawn` を trait 化し、環境差を吸収する:

```rust
pub trait Mailbox<M> {
  fn try_send(&self, msg: M) -> Result<(), ()>;
  fn try_recv(&self) -> Option<M>;
}

pub trait Spawn {
  fn spawn(&self, fut: impl core::future::Future<Output = ()> + 'static);
}
```

- std 版: `tokio::mpsc` やロックフリー MPMC を内部実装として採用。
- embedded 版: `heapless::spsc::Queue` や `bbqueue` などのシングルコア向けバッファを使用。

## 環境ごとの所有モデルまとめ

| 環境              | 所有モデル          | 並行モデル                     | 備考                          |
|-------------------|---------------------|--------------------------------|-------------------------------|
| std (サーバ)       | `Arc<T>`            | OS スレッド / tokio            | `Send` + `Sync` 前提          |
| RP2350 (Cortex-M33)| `Arc<T>`            | デュアルコア / embassy         | 原子命令あり                  |
| RP2040 (Cortex-M0+)| `Rc<T>` or `StaticRef<T>` | シングルコア async / 割り込み | 原子命令なし。`Rc` は単核専用 |

## StateCell 実装ポリシー
- `RcStateCell<T>`: `Rc<RefCell<T>>` を内部に保持し、`embedded_rc` フィーチャで利用する。シングルスレッド前提で `RefCell` による実行時借用チェックを活用する。
- `ArcStateCell<T>`（std）: `Arc<std::sync::Mutex<T>>` を内部に保持し、Tokio ランタイム上で同期的に状態を書き換える用途に用いる。`actor-std` のプレリュードから利用可能。
- `ArcStateCell<T, RM = NoopRawMutex>`（embedded）: `Arc<embassy_sync::mutex::Mutex<RM, T>>` を利用し、`embedded_arc` フィーチャで提供する。既定では `ArcLocalStateCell<T>` により `NoopRawMutex` を採用して単一エグゼキュータ向け最小実装とし、実機で割り込みやマルチコアを扱う場合は `ArcCsStateCell<T>` を使って `CriticalSectionRawMutex` を選択する。必要に応じて他の `RawMutex` へも切り替えられる。
- `StateCell` トレイトは `borrow` / `borrow_mut` を同期 API として提供しており、Actor 本体は単一スレッドでメッセージ処理を行う前提に立つ。今後 async ロックが必要な場合は `StateCell` 拡張メソッドに `try_borrow_async` 等を追加する余地を残す。

## Queue 抽象の配置
- `nexus-utils-core-rs` に `RingBuffer` と `QueueBase` 系トレイトを集約し、`no_std + alloc` 前提でリングキューの基盤を提供する。
- MPSC リング実装向けの `RingBufferStorage` トレイトも `collections::queue::storage` に集約し、`RingBufferBackend` と共有する。
- リングキューは `RingBackend` / `RingStorageBackend` を介して動作し、MPSC と同様に Backend + Handle + Queue の三層構成へ統一する。
- `nexus-utils-std-rs` では `RingQueue<E>` を `Arc<Mutex<_>>` でラップし、共有アクセス向け API（`offer` / `poll` など）を公開する。
- `nexus-utils-embedded-rs` では `RcRingQueue<E>` と `ArcRingQueue<E, RM>` を用意し、`rc` フィーチャでは `Rc<RefCell<_>>`、`arc` フィーチャでは Embassy の `Mutex<RM, _>` を使用する。`ArcLocalRingQueue` / `ArcCsRingQueue` などのエイリアスで環境に合わせた排他制御を選択しつつ、ロジックは core の共有実装へ委譲する。
- 両クレートは `prelude` モジュールで `QueueWriter` / `QueueRw` など core 側のトレイトを含む共通インターフェイスを再エクスポートし、利用者が core の API だけで操作できるよう保証する。
- `actor-*` 側はユーティリティ経由の再エクスポートを利用し、std / embedded いずれの構成でも一貫したテスト・サンプルを保つ。

### MPSC バックエンド再設計計画（2025-10-05 更新）
1. **抽象名と API の刷新**
   - 現行の `MpscStorage` を `MpscBackend`（仮称）へ改名し、`try_send` / `try_recv` / `close` / `len` / `capacity` 等のトランスポート指向 API に整理する。
  - `MpscQueue` は新 backend を委譲するだけの薄いラッパへ再設計し、プラットフォーム固有の詳細を背後へ隠蔽する。
2. **リングバッファ backend の再配置**
   - 既存のリングバッファ実装を `core` 上の `MpscBackend` 1 実装として切り出し、std / embedded からは backend 指定のみで利用できるようにする。
  - `QueueWriter` / `QueueReader` / `QueueRw` を通じてアクセスする既存 API は維持し、内部で利用する backend だけを差し替える。
3. **Tokio backend の追加**
   - `tokio::sync::mpsc` を包む `TokioMpscBackend` を `utils-std` に実装し、`ArcMpsc*` 系型は backend 選択に専念させる。
   - 将来 `Tokio` 以外のランタイム（例: `async-std`）へ拡張する余地を残し、backend の差し替えで対応できるようにする。
4. **Embassy backend 拡張**
   - RP2350 向けに `embassy_sync::channel::Channel` を包む backend を検討し、`embedded_arc` フィーチャで切り替え可能とする。
   - RP2040 向けの `Rc` 基盤も同一トレイトで運用し、バックエンドの違いを `core` で統一的に扱う。
5. **テスト & カバレッジ**
   - backend 差し替えが同一振る舞いを提供することを確認する統合テストを `utils-core` に追加する。
   - `std` / `embedded` では `core` の振る舞いをモック backend で検証し、プラットフォーム依存コードは薄いアダプタ部のテストに限定する。
   - `./coverage.sh` に backend 差し替えテストを組み込み、全モジュール 80% 以上のラインカバレッジを維持する。

### コード配置とテスト方針（共通原則）
- 可能な限り 1 ファイル 1 型を基本とし、複数型を同一ファイルに置く場合は密接に関連する実装のみに留める。論理的なまとまりは `pub use` を活用して表現する。
- コアとなる振る舞いは `utils-core` / `actor-core` に集中させ、`std` / `embedded` は依存プラットフォームへ接続する薄い適合レイヤとする。
- 単体テストは `core` 層に集約し、`std` 側でモックや backend 差し替えを用いた検証を実行する。embedded 固有のコードは PC 上でも動作するモック（std の `critical-section` 実装など）を用意し、必要最低限の追加テストで済む構造を維持する。
- カバレッジ目標は全モジュール 80% 以上とし、`cargo test --workspace` と `cargo test -p nexus-utils-embedded-rs --no-default-features --features arc` を含む計測スクリプトを維持する。

### Stack 抽象
- LIFO 構造についても `StackBuffer<T>`（core）と `Stack<H, T>` を導入し、`Queue` と同等の共有モデルで扱えるようにする。
- `StackStorage` / `StackHandle` を実装することで、`Arc<Mutex<_>>` や `Rc<RefCell<_>>`、`ArcStateCell<_>` を透過的に利用できる。
- std 向けには `ArcStack<T>`（内部は `ArcShared<Mutex<_>>`）、embedded 向けには `RcStack<T>` / `ArcStack<T, RM>` を提供し、`prelude` から再エクスポートすることで core の `StackBase` / `StackMut` API 経由で操作可能にする。
- core に振る舞いを集中させるのが基本方針。`StackBuffer` や `Stack` のロジックはすべて core に置き、std 側（`Arc` + `Mutex`）で単体テストを回せば大半の品質をカバーできる。embedded 側は所有モデルだけを差し替え、std で通したテストケースを再利用することで最小限の追加検証で済む構造とする。

## モジュール構成ポリシー
- `actor-std` は `spawn.rs` / `timer.rs` / `mailbox.rs` / `state.rs` に分割し、Tokio 依存の責務を明確化して再エクスポート専用の `lib.rs` から集約する。
- `actor-embedded` も同様に `spawn.rs` / `timer.rs` / `mailbox.rs` / `state.rs` / `shared.rs` へ分離し、feature ごとの実装差分をファイル単位で追えるようにした。
- これにより std / embedded の両環境で MECE な検証対象を保ちつつ、今後の拡張（例: Embassy バリエーションや追加ランタイム）を小さなモジュール追加で進められる。

## 運用上の考慮
- `Shared<T>` の実装ごとに `Send` / `Sync` 制約が変わるため、`Shared` トレイトや利用側で適切な境界を設ける。
- `Arc` → `Shared` への置換は ActorRef / Context / Mailbox / Scheduler / Runtime 全域に波及するため、段階的なリファクタリング計画が必要。
- 既存テストを維持しつつ進めるため、小さな PR 単位で進行させる。
- Feature 組み合わせが増加するため、CI で主要パス（`std`, `embedded_rc`, `embedded_static`）をカバーする体制を準備する。

## 今後の進め方（初期ステップ）
1. **Shared 抽象導入の足場づくり**
   - `Shared<T>` トレイトと代表的な実装（`Arc` / `Rc` / `StaticRef`）を追加し、段階的に既存コードへ適用できるようにする。
   - まずは `core` 層で `Arc` を直接持っている箇所を洗い出し、`Shared` を通すためのジェネリクス化が妥当か、API を再設計すべきかを分類する。

2. **基盤コンポーネントのジェネリクス化**
   - ActorRef / Context / Mailbox / Scheduler / Runtime など、共有所有権を扱うコア API を `Shared` ベースに書き換える。
   - この際、`Send`/`Sync` 制約の有無や `try_unwrap` の扱いなど、環境差異を吸収するための追加メソッドが必要か検討する。

3. **環境別フィーチャの実装**
   - `std` 構成では従来通り `Arc` を用い、既存テストがすべて通る状態を維持しながら進める。
   - `embedded_rc` フィーチャを新設し、RP2040（Cortex-M0+）向けビルドで `Rc` を利用できるようにする。`alloc` あり／`no_std` 前提のビルドパスと CI を整える。
   - `embedded_static` など追加モードも検討し、静的メモリ向けの `StaticRef` 実装を整備する。
   - `embedded_arc` フィーチャでは Arc を利用し、RP2350 のような atomic 対応 MCU で Embassy の `Channel` などを活用できるようにする。

4. **Mailbox / Spawn 抽象の切り替え**
   - `Mailbox` や `Spawn` のトレイト化を進め、std 版（tokio）と embedded 版（heapless 等）の実装差を吸収する。
   - 関連するテストとベンチマークを `Shared` ベースに更新する。

5. **CI & ドキュメント整備**
   - 主要フィーチャ組み合わせ（`std`, `embedded_rc`, `embedded_static`）を CI で継続的に検証する。
   - `AGENTS.md` など運用ドキュメントに、所有モデルを切り替える手順とフィーチャ一覧を明記する。

6. **段階的リリース計画**
   - 既存の `std` / `Arc` 構成を壊さないよう小さなステップで進める。
   - 各マイルストーンで `cargo test --workspace` とクロスビルドを実施しながら、段階的に `Shared` ベースへ移行する。

## 優先度
- 本設計は `embedded` モジュールで Arc が利用できない MCU をサポートする唯一の実現手段であり、最優先で進める。
- 既存実装の裁量範囲が広く、長期のリファクタリングとなるため、マイルストーンと CI 整備を合わせて進行する。

## 参考モデルの扱いに関して
- 上記で示した async モデルのコード断片（std/tokio・RP2040・RP2350 向け）は、所有モデル抽象の考え方を共有するための参考資料である。
- 実際の実装では、性能・安全性・周辺コンポーネントとの整合を考慮し、これらをそのまま適用せずに最適な形で再設計すること。
- 設計アイデアとして参照しつつ、必要な箇所はモジュール構成や抽象を含めて見直す。

## RP2040 向け動作確認サンプル
- `modules/actor-embedded/examples/rp2040_basic.rs` では、`LocalMailbox` / `ImmediateTimer` を使った最小構成の actor を RP2040 上で動かす例を提供する。
- ビルド手順（事前に `rustup target add thumbv6m-none-eabi` を実行）
  ```bash
  cargo build -p nexus-actor-embedded-rs --example rp2040_basic \
    --target thumbv6m-none-eabi --no-default-features --features embedded_rc
  ```
- 例では `alloc_cortex_m`, `cortex-m-rt`, `panic-halt` を利用してヒープ初期化とエントリポイントを用意し、`actor_loop` を単一メッセージで動作確認する。実際のアプリケーションでは Embassy 等の実行器やハードウェアタイマに差し替える。

- UF2 変換 (`elf2uf2-rs`) についての注意: 生成された ELF の OS/ABI フィールドが `GNU/Linux (0x3)` になっているため、現行の `elf2uf2-rs` は "Unrecognized ABI" エラーを返す。回避策としては以下のいずれかを採用する。
  - 公式 SDK の `uf2conv.py` など別ツールを利用する。
  - `elf2uf2-rs` にパッチを当てて ABI チェックを外し、ローカルで再ビルドする。
  - LLVM リンカ設定で OSABI を 0 (`System V`) に修正できないか調査する（要検討）。

- 目視確認用に `modules/actor-embedded/examples/rp2040_led.rs` も用意。以下の手順でビルド／書き込みするとオンボード LED (GP25) が点滅し、新しいファームの動作を確認できる。
  ```bash
  cargo build -p nexus-actor-embedded-rs --example rp2040_led \
    --target thumbv6m-none-eabi --no-default-features --features embedded_rc --release

  python3 scripts/uf2conv.py target/thumbv6m-none-eabi/release/examples/rp2040_led \
    -o target/thumbv6m-none-eabi/release/examples/rp2040_led.uf2 \
    --family RP2040 --base 0x10000000

  picotool load target/thumbv6m-none-eabi/release/examples/rp2040_led.uf2 -x
  ```

## RP2350 向け動作確認サンプル
- `modules/actor-embedded/examples/rp2350_arc.rs` は `ArcStateCell`（既定 `NoopRawMutex`）を利用した最小構成を示し、RP2350（Cortex-M33 クラス）で `embedded_arc` を有効化した際の動作確認に用いる。
- ビルド例（thumbv8m/main ターゲットを事前に追加しておく）：
  ```bash
  rustup target add thumbv8m.main-none-eabihf
  cargo build -p nexus-actor-embedded-rs --example rp2350_arc \
    --target thumbv8m.main-none-eabihf --no-default-features --features alloc,embedded_arc
  ```
- 実機で割り込み共存を行う場合は `ArcCsStateCell::new` を採用し、`critical-section` 実装（例: `cortex-m` の `critical-section-single-core`）を初期化する。CI では少なくとも上記クロスビルドを追加し、`embedded_arc` のビルド破綻を検知するようジョブを拡張する。

## Rust ファイル構成ルール（2025-10-05 追加）
- 1 ファイル 1 役割を原則とし、公開構造体やバックエンド実装が複数存在する場合は `queue/mpsc/backend.rs` のように責務ごとのサブモジュールへ分割する。
- `mod.rs` は利用せず、ディレクトリ直下の `mod.rs` が既存に残る場合は再エクスポート専用ファイルとして扱い、実装本体は `foo.rs` / `bar.rs` に配置する。
- 共有アクセス用の API 命名は引数の参照形で統一する：`offer_mut`（`&mut self`）、`offer`（`&self`）。`*_shared` や `try_offer` といった冗長なサフィックスは使用しない。
- テストは同一ファイルの `#[cfg(test)] mod tests` にまとめ、補助ハンドルや `Shared` 実装はテスト用モジュール内でローカル定義する。複数のテストから使い回すヘルパは `tests` モジュール配下の関数で提供する。
- インポート順序は「標準ライブラリ → 外部クレート → ワークスペース内クレート → ローカルモジュール」を基本とし、`use crate::…` / `use super::…` を混在させない。再エクスポート (`pub use`) はモジュール末尾にまとめる。
- Feature 切り替えが必要な場合はファイル単位で `cfg` を付与し、単一ファイルに `#[cfg]` ブロックが乱立しないよう留意する。環境差が大きい箇所は `foo/std.rs` / `foo/embedded.rs` のようにファイルを分割して `mod` で切り替える。
