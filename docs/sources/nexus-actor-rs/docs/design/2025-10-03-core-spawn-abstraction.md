# CoreSpawn 抽象設計メモ (2025-10-03)

## 区分基準
- **現状**: actor-core / actor-std / actor-embedded が依存する実行環境の違い整理。
- **要件**: no_std + executor 非依存を維持しつつ、Tokio / Embassy 双方で利用できる抽象の定義。
- **対応案**: 新規トレイトと API 変更案。

## 現状整理
- `CoreRuntimeConfig` は `Timer` / `CoreScheduler` / `AsyncYield` / `FailureClock` を保持しているが、タスク起動（spawn）自体は各実装依存 (`tokio::spawn`, `ActorSystem::spawn`) に委ねられている。
- `actor-std` では `tokio::spawn` が標準。Embassy では `Spawner::spawn` で `!Send` Future を許容するケースもある。
- `CoreSpawnMiddleware` はコンテキスト内部の同期操作であり、executor との橋渡しは提供していない。

## 要件
1. **Future 種別**: spawn する Future は `Send + 'static` を基本とし、Embassy の `!Send` ノード対応は別フィーチャで検討。
2. **エラー処理**: spawn が失敗する場合（リソース不足など）を `Result` で返す。
3. **ハンドル**: キャンセル/await が必要であれば `JoinHandle` 的抽象を用意（最低限キャンセルは必須）。
4. **Integrate with Scheduler**: `CoreScheduler` から生成されるタスクも spawn 抽象を経由させると API 統一が図れる。

## 対応案
- 新トレイト `CoreSpawner` を `modules/actor-core/src/runtime.rs` に追加。
  ```rust
  pub trait CoreSpawner: Send + Sync + 'static {
      type JoinHandle: CoreJoinHandle;
      fn spawn(&self, task: CoreTaskFuture) -> Result<Self::JoinHandle, CoreSpawnError>;
      fn scope(&self) -> CoreSpawnScope;
  }
  ```
  - `CoreSpawnScope` は `Send` 要件緩和の検討用に `enum CoreSpawnScope { Local, Send }` を想定（初期実装では `Send` のみ）。
  - `CoreJoinHandle` トレイトを新設し、`cancel`/`is_finished`/`detach` などのメソッドを定義。Tokio の `JoinHandle` と Embassy の `SpawnToken` を包むラッパで実装。
- `CoreRuntimeConfig` に `spawner: Arc<dyn CoreSpawner>` を追加し、`CoreRuntime` から `spawner()` アクセサを提供する。`with_spawner` を追加しビルダー的に設定可能にする。
- `CoreScheduler` の `schedule_*` 実装は内部で `CoreSpawner::spawn` を利用してタスクを起動し、戻り値として `CoreScheduledHandleRef` を返却する。この際 `CoreScheduledHandleRef` を `CoreJoinHandle` を包む型として再設計する。
- `CoreSpawnError`
  ```rust
  pub enum CoreSpawnError {
      ExecutorUnavailable,
      CapacityExhausted,
      Rejected(&'static str),
  }
  ```
  - `Display`/`Error` 実装を提供し、std/embedded 双方で軽量なエラー表現を確保。

## 実装スケッチ
- `actor-std`: `TokioSpawner` が `tokio::runtime::Handle::current().spawn(task)` を呼び出し、`JoinHandle<()>` を返却。
- `actor-embedded`: `EmbassySpawner` が `embassy_executor::Spawner` を保持し、`Spawner::spawn(task).map_err(...)` を実行。
- `remote-core` 等は `CoreRuntime::spawner()` を通じて spawn 抽象へアクセスする。

### actor-std 実装方針
- 追加モジュール案: `modules/utils-std/src/runtime/spawn.rs`
  ```rust
  pub struct TokioCoreSpawner {
      handle: tokio::runtime::Handle,
  }
  ```
  - `TokioCoreSpawner::spawn` は `handle.spawn(task)` を呼び、`TokioJoinHandle` を返す。
  - `TokioJoinHandle` は `tokio::task::JoinHandle<()>` を内包し、`CoreJoinHandle` を実装:
    - `fn cancel(&self)` → `handle.abort()`
    - `fn is_finished(&self)` → `handle.is_finished()`
    - `fn detach(self)` → `drop(self)`
- `modules/utils-std/src/runtime/sync.rs:183` の `tokio::spawn(future)` を `TokioCoreSpawner::spawn` 経由に置き換える。
- `actor-std` の起動コード（例: `modules/actor-std/src/actor/system.rs`）で `CoreRuntimeConfig::with_spawner(Arc::new(TokioCoreSpawner::new(tokio::runtime::Handle::current())))` を適用。
- PoC テスト: `modules/utils-std/src/runtime/tests.rs` に CoreSpawner 経由でタスクが起動し `JoinHandle` が正常に完了することを検証する非同期テストを追加。

### actor-embedded 実装方針
- 追加モジュール: `modules/actor-embedded/src/spawn.rs`
  ```rust
  pub struct EmbassyCoreSpawner<'a> {
      spawner: embassy_executor::Spawner<'a>,
  }
  ```
  - `spawn` は `spawner.spawn(task).map_err(|_| CoreSpawnError::ExecutorUnavailable)`。
  - 現時点では `Send` Future のみを許可し、`!Send` 対応は `feature = "local-spawn"` で後日検討。
  - JoinHandle は `SpawnToken<()>` を包み、`cancel` は `token.cancel()` を呼び出す。
- Embassy のライフタイム制約により `'static` な Spawner 共有が難しいため、`CoreRuntimeConfig` で `Arc<dyn CoreSpawner>` を保持する際に `Arc<Mutex<Option<EmbassyCoreSpawner>>>` などを活用する案を検討。
- PoC: `examples/embedded_blink.rs` で `EmbeddedRuntimeBuilder::build_runtime()` を呼び出し、spawn → cancel のパスを確認。
## 影響範囲とステップ
1. **actor-core**
   - `CoreRuntimeConfig` 構造体に `spawner` フィールドと `with_spawner` メソッドを追加。
   - `CoreRuntime` へ `spawner()` アクセサを追加。
   - 既存呼び出し元に影響しないよう、従来の `new(timer, scheduler)` は `spawner` にデフォルト `Arc<NoopSpawner>` を設定する暫定実装を用意。
2. **utils-std / actor-std**
   - `TokioCoreSpawner` を定義し、`tokio::runtime::Handle` を保持。
   - `TokioCoreSpawner::JoinHandle` は `tokio::task::JoinHandle<()>` を包むラッパ構造体で `CoreJoinHandle` を実装。
   - 主要箇所 (`modules/utils-std/src/runtime/sync.rs` など) の `tokio::spawn` を spawner 経由へ移行。
3. **utils-embedded / actor-embedded**
   - `EmbassyCoreSpawner` を実装 (`embassy_executor::Spawner` + `'static` Future のみ対応)。
   - `spawn()` で `Spawner::spawn` を呼び、返り値 `SpawnToken<()>` を包む。
4. **段階的な移行**
   - 本番コード → テストコードの順で `tokio::spawn` を置き換え。
   - `remote-std` や `cluster-std` の長時間タスクは優先度高。テスト内の `tokio::spawn` は最後に置き換え、または spawner から直接ハンドルを取り出すユーティリティを提供。

## TODO
- `CoreScheduler` と `CoreSpawner` の組み合わせを検証する PoC を `actor-core/tests` に追加。
- `CoreSpawnMiddleware` が新トレイトと重複しないか確認し、必要なら命名整理。
