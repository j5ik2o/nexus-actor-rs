# 所有モデル抽象化による std / embedded 両対応計画 (2025-10-04)

## 方針概要
- `core` クレートは `no_std + alloc` を前提とし、OS / スレッド / `Arc` に依存しない純粋ロジック層を構築する。
- `std` モジュールはサーバ／Linux 環境を対象とし、`Arc`・`Send`・`Sync`・`tokio` 等でフル機能化する。
- `embedded` モジュールは MCU 向けとし、`Arc` に頼らず `Rc` または `StaticRef` を利用する所有モデルを採用する。
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
default = ["std"]
std = []
embedded_rc = []      # RP2040 等、シングルコア / 非アトミック環境
embedded_static = []  # `StaticRef` による完全 static 構成
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
