# actor-core Shared 抽象への移行メモ

## 目的
- `actor-core` が `alloc::sync::Arc` や CAS 前提の構造に強く依存しており、`thumbv6m-none-eabi` (RP2040) のような CAS 非対応環境でビルドできない。
- `nexus-utils-core-rs` が提供する `Shared`/`StateCell` 抽象を軸に、`Arc`／`Rc` バックエンドを差し替え可能な形に再設計する。

## Arc 利用箇所の分類
| カテゴリ | 代表ファイル / 型 | 備考 |
|----------|--------------------|------|
| クロージャ共有（`Arc<dyn Fn…>`） | `api/actor/behavior.rs`, `api/actor/props.rs`, `runtime/context/actor_context.rs`, `runtime/supervision/...` | `MapSystemFn`, `BehaviorFactory`, メッセージアダプタ、失敗ハンドラなど |
| Factory 共有（`Arc<dyn ReceiveTimeoutSchedulerFactory<…>>`） | `api/actor/system.rs`, `runtime/scheduler/actor_cell.rs`, `priority_scheduler.rs` | 受信タイムアウトスケジューラの DI |
| 共有状態（`Arc<AtomicBool>` など） | `api/actor/system.rs` (ShutdownToken), `api/actor/ask.rs` (AskShared) | CAS を直接使用 |
| イベント／エスカレーション（`Arc<dyn Fn>`） | `api/event_stream.rs`, `api/supervision/escalation.rs` | 失敗イベントの購読など |
| テスト・補助コード | `tests.rs`, `benches/metadata_table.rs` 等 | 影響度低 |

## 移行方針
1. **Shared 抽象の整備**
   - `Shared<dyn Fn…>` を扱いやすくするため、`ArcSharedFn` / `RcSharedFn` のようなラッパーを util 層（`nexus-utils-core-rs`）に追加し、関数ポインタ共有を DIP 化する。
   - Factory 系 (`ReceiveTimeoutSchedulerFactory`) も同様に `Shared` ベースへ。

2. **actor-core の型置換**
   - `Arc<MapSystemFn<…>>` 等を `Shared` ベースの alias（例: `MapSystemShared`) に変更。
   - `Props` / `Behavior` / `MessageEnvelope` などの API が `Arc` を露出しないように修正。

3. **共有状態の再設計**
   - `Arc<AtomicBool>`（ShutdownToken）を `StateCell<bool>` + `critical_section` / `portable_atomic` へ置換。
   - Ask 実装（`Arc<AskShared<_>>` + `AtomicWaker`）を `Shared` + `portable_atomic` or `critical_section` ベースにリファクタ。

4. **段階的移行ステップ**
   1. 既存 API から `Arc` が露出している箇所を `Shared` alias に差し替え、`ArcShared` を暫定実装として利用。
   2. 内部実装で `Arc::new` / `Arc::clone` を使っている部分を `ArcShared` 経由に書き換え。
   3. `Rc` バックエンド（`RcShared`）でも同一コードが動くように feature 設計を整備。
   4. `thumbv6m-none-eabi` を含むターゲットでビルド＆動作確認。

## TODO（初期スコープ）
- [ ] `nexus-utils-core-rs` に Shared 系ラッパー (`SharedFn`, `SharedFactory` のような型) を追加する。
- [ ] `actor-core` 内の `Arc` 使用箇所を該当ラッパーに置換する。
- [ ] `ShutdownToken`・ASK 周りの共有状態を `StateCell`／`portable_atomic` ベースに移行。
- [ ] RP2040 でのビルド確認（`thumbv6m-none-eabi`）を CI/ローカル双方で実施。
