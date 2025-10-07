# Embassy 向けディスパッチループ統合メモ (2025-10-07)

## 目的
`nexus-actor-embedded-rs` の `spawn_embassy_dispatcher` ヘルパを利用し、Embassy executor 上で
`ActorSystem` のディスパッチループを常駐させる手順をまとめる。

## 必要条件
- `nexus-actor-embedded-rs` で `embassy_executor` フィーチャを有効化する。
- `embassy-executor` / `embassy-sync` が利用側プロジェクトで初期化済みであること。
- `ActorSystem` を `'static` な領域（例: `StaticCell`）に配置できること。

## 手順
1. Cargo フィーチャを有効化する。
   ```toml
   nexus-actor-embedded-rs = { version = "*", features = ["embassy_executor"] }
   ```
2. `StaticCell` などで `ActorSystem` を確保し、`LocalMailboxRuntime` など適切なランタイムを渡して初期化する。
3. 初期化した `ActorSystem` の可変参照を `spawn_embassy_dispatcher` に渡し、Embassy `Spawner` に常駐タスクとして登録する。
4. 以降は通常どおり `root_context()` からアクターを起動すれば、Embassy タスクが自動的に `dispatch_next` を駆動する。

## サンプルコード
```rust
use embassy_executor::Spawner;
use static_cell::StaticCell;
use nexus_actor_core_rs::{ActorSystem, MailboxOptions};
use nexus_actor_embedded_rs::{LocalMailboxRuntime, spawn_embassy_dispatcher};

static SYSTEM: StaticCell<ActorSystem<MessageEnvelope<u32>, LocalMailboxRuntime>> = StaticCell::new();

pub fn start(spawner: &Spawner) {
    let runtime = LocalMailboxRuntime::default();
    let system = SYSTEM.init_with(|| ActorSystem::new(runtime));

    // Embassy タスクとしてディスパッチを起動
    spawn_embassy_dispatcher(spawner, system).expect("spawn dispatcher");

    // 以降、TypedRootContext を通じてアクターを起動
    let mut root = system.root_context();
    // ... root.spawn(...)
}
```

## 今後の TODO
- Embassy 用タイマー／シグナル実装を共通化し、`spawn_child` から直接 Embassy の I/O を扱えるようにする。
- `spawn_embassy_dispatcher` がエラー終了した際の通知経路（ログ・イベントベース）の整備。
