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
1. `Shared<T>` トレイトと代表的な実装を導入し、既存 `Arc` ベースのコードを `Shared` に置き換える。
2. Actor / Scheduler / Mailbox など主要コンポーネントで `Shared` を透過させるための API 調整を行う。
3. `embedded_rc` フィーチャを作成し、RP2040 ターゲットで `Rc` ベースのビルドを確認する。
4. `embedded_static` など追加モードや `RP2350` 向けの `Arc` モードを順次整備する。

## 優先度
- 本設計は `embedded` モジュールで Arc が利用できない MCU をサポートする唯一の実現手段であり、最優先で進める。
- 既存実装の裁量範囲が広く、長期のリファクタリングとなるため、マイルストーンと CI 整備を合わせて進行する。
