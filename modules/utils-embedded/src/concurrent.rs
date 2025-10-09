//! 並行処理プリミティブモジュール。
//!
//! このモジュールは、`no_std` 環境で使用できる並行処理および同期プリミティブを提供します。
//! `Rc` または `Arc` ベースの実装を、フィーチャフラグで選択できます。
//!
//! # 提供される同期プリミティブ
//!
//! - **AsyncBarrier**: 複数のタスクが同期ポイントで待機するバリア
//! - **CountDownLatch**: カウントが 0 になるまで待機するラッチ
//! - **WaitGroup**: 複数のタスクの完了を追跡する待機グループ
//! - **Synchronized**: 排他的アクセス制御（Mutex ベース）
//! - **SynchronizedRw**: 読み取り/書き込みアクセス制御（RwLock ベース）
//!
//! # フィーチャフラグ
//!
//! - **`rc`**: `Rc` ベースの実装（シングルスレッド専用）
//! - **`arc`**: `Arc` ベースの実装（マルチスレッド対応）
//!   - `ArcLocal*`: `Arc` + `LocalMutex`/`LocalRwLock`（シングルスレッド最適化）
//!   - `ArcCs*`: `Arc` + `CsMutex`/`CsRwLock`（クリティカルセクションベース）
//!   - `Arc*`: `Arc` + 標準 Mutex/RwLock

#[cfg(feature = "rc")]
mod rc_synchronized;
#[cfg(feature = "rc")]
pub use rc_synchronized::{
  RcMutexBackend, RcRwLockBackend, Synchronized as RcSynchronized, SynchronizedRw as RcSynchronizedRw,
};
#[cfg(feature = "rc")]
mod rc_count_down_latch;
#[cfg(feature = "rc")]
pub use rc_count_down_latch::{CountDownLatch as RcCountDownLatch, RcCountDownLatchBackend};
#[cfg(feature = "rc")]
mod rc_wait_group;
#[cfg(feature = "rc")]
pub use rc_wait_group::{RcWaitGroupBackend, WaitGroup as RcWaitGroup};
#[cfg(feature = "rc")]
mod rc_async_barrier;
#[cfg(feature = "rc")]
pub use rc_async_barrier::{AsyncBarrier as RcAsyncBarrier, RcAsyncBarrierBackend};

#[cfg(feature = "arc")]
mod arc_synchronized;
#[cfg(feature = "arc")]
pub use arc_synchronized::{
  ArcCsSynchronized, ArcCsSynchronizedRw, ArcLocalSynchronized, ArcLocalSynchronizedRw, ArcMutexBackend,
  ArcRwLockBackend, ArcSynchronized, ArcSynchronizedRw,
};
#[cfg(feature = "arc")]
mod arc_count_down_latch;
#[cfg(feature = "arc")]
pub use arc_count_down_latch::{ArcCountDownLatchBackend, ArcCsCountDownLatch, ArcLocalCountDownLatch};
#[cfg(feature = "arc")]
mod arc_wait_group;
#[cfg(feature = "arc")]
pub use arc_wait_group::{ArcCsWaitGroup, ArcLocalWaitGroup, ArcWaitGroupBackend};
#[cfg(feature = "arc")]
mod arc_async_barrier;
#[cfg(feature = "arc")]
pub use arc_async_barrier::{ArcAsyncBarrierBackend, ArcCsAsyncBarrier, ArcLocalAsyncBarrier};
