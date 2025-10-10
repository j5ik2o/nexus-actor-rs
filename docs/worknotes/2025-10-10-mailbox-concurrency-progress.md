# Mailbox Concurrency Progress (2025-10-10)

## Step 1: Concurrency Marker Introduction ✅
- Added `MailboxConcurrency` trait and `ThreadSafe` / `SingleThread` markers in `modules/actor-core/src/runtime/mailbox/traits.rs`.
- Updated `MailboxFactory` with new associated type `type Concurrency`.
- Tokio, Local, Arc, and Test mailbox factories now declare `type Concurrency = ThreadSafe`.
- Re-exported `ThreadSafe` / `SingleThread` from `nexus_actor_core_rs` via `modules/actor-core/src/lib.rs`.
- `cargo check --workspace` passes (2025-10-10).

## Step 2: Metadata + Sender Generalization ✅
- `InternalMessageSender` と `MessageSender` を並行性マーカー `C` でパラメータ化し、`MailboxFactory::Concurrency` と連携できるようにした。
- `MessageMetadata<C>` / `InternalMessageMetadata<C>` を導入し、`Context` / `ActorRef` / `Props` など全経路で `R::Concurrency` を伝搬。
- メタデータテーブルを `StoredMessageMetadata` ベースに再設計し、`MetadataStorageMode` トレイトで ThreadSafe / SingleThread 双方を保管可能にした。
- `create_ask_handles` を `MailboxConcurrency` 汎用化、ask 系 API とベンチ・テストを新しい型パラメータに合わせて更新。
- `MailboxFactory::type Concurrency` に `MetadataStorageMode` 制約を追加し、ファクトリ側で必要な境界を自動的に満たすようにした。
- `cargo check --workspace` / `cargo test --workspace` を実行し成功を確認（2025-10-10）。
- `cargo bench -p nexus-actor-core-rs --features std --bench metadata_table` を追加計測し、`side_table` は ThreadSafe ≒34ns / SingleThread ≒34ns、`inline` は ThreadSafe ≒16ns / SingleThread ≒17ns と概ね同等性能を確認（2025-10-10）。
- `cargo build -p nexus-actor-embedded-rs --target thumbv6m-none-eabi --no-default-features --features alloc,embedded_rc` を実行し、`embedded_rc` 構成でもコンパイル成功することを確認（2025-10-10）。

## embedded_rc ビルドメモ（RP2040 等）

```
cargo build -p nexus-actor-embedded-rs \
  --target thumbv6m-none-eabi \
  --no-default-features \
  --features alloc,embedded_rc
```

- `thumbv6m-none-eabi` ターゲットを事前に `rustup target add` で導入しておく。
- シングルスレッド環境では `LocalMailboxFactory` が `SingleThread` を宣言し、`Rc` バックエンドで動作。
- `EmbeddedFailureEventHub` は `Rc<Mutex<...>>` を内部で保持しているため、Send/Sync は `#[cfg]` 下で `unsafe impl` している（割り込み環境での利用時は critical section で守る前提）。

## Notes
- 今後の Step 3 以降はメタデータ ストレージの最適化や SingleThread ファクトリの実装に着手できる状態。
