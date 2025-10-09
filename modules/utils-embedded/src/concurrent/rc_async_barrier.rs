use alloc::boxed::Box;
use alloc::rc::Rc;

use core::cell::RefCell;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::signal::Signal;
use nexus_utils_core_rs::{async_trait, AsyncBarrier as CoreAsyncBarrier, AsyncBarrierBackend};

/// `Rc` ベースの非同期バリア実装のバックエンド。
///
/// `no_std` 環境で、複数のタスクが特定の同期ポイント（バリア）で待機・再開する機能を提供します。
/// 内部では `RefCell` と Embassy の `Signal` を使用してシングルスレッド同期を実現します。
///
/// # 特徴
///
/// - `Rc` による参照カウント（シングルスレッド専用）
/// - Embassy の `NoopRawMutex` による軽量な同期
/// - カウントがゼロになると全てのタスクが同時に解放される
///
/// # 使用例
///
/// ```ignore
/// let barrier = AsyncBarrier::new(2);
/// let other = barrier.clone();
///
/// let first = barrier.wait();
/// let second = other.wait();
///
/// join!(first, second); // 両方のタスクが同時に進行
/// ```
#[derive(Clone)]
pub struct RcAsyncBarrierBackend {
  remaining: Rc<RefCell<usize>>,
  initial: usize,
  signal: Rc<Signal<NoopRawMutex, ()>>,
}

#[async_trait(?Send)]
impl AsyncBarrierBackend for RcAsyncBarrierBackend {
  /// 指定されたカウントで新しいバリアバックエンドを作成します。
  ///
  /// # パニック
  ///
  /// `count` が 0 の場合はパニックします。
  fn new(count: usize) -> Self {
    assert!(count > 0, "AsyncBarrier must have positive count");
    Self {
      remaining: Rc::new(RefCell::new(count)),
      initial: count,
      signal: Rc::new(Signal::new()),
    }
  }

  /// バリアで待機します。
  ///
  /// すべての参加者（`count`個のタスク）が `wait()` を呼び出すまでブロックします。
  /// 最後のタスクがバリアに到達すると、全てのタスクが同時に解放されます。
  ///
  /// # パニック
  ///
  /// `wait` が `count` 回を超えて呼び出された場合はパニックします。
  async fn wait(&self) {
    let remaining = self.remaining.clone();
    let signal = self.signal.clone();
    let initial = self.initial;
    {
      let mut rem = remaining.borrow_mut();
      assert!(*rem > 0, "AsyncBarrier::wait called more times than count");
      *rem -= 1;
      if *rem == 0 {
        *rem = initial;
        signal.signal(());
        return;
      }
    }

    loop {
      signal.wait().await;
      if *remaining.borrow() == initial {
        break;
      }
    }
  }
}

/// `Rc` ベースの非同期バリアの型エイリアス。
///
/// `no_std` 環境で使用できる非同期バリア実装です。
/// 複数のタスクが同期ポイントで待機し、全員が揃うまで待つ機能を提供します。
pub type AsyncBarrier = CoreAsyncBarrier<RcAsyncBarrierBackend>;

#[cfg(test)]
mod tests {
  use super::AsyncBarrier;
  use futures::executor::block_on;
  use futures::join;

  #[test]
  fn barrier_releases_all() {
    block_on(async {
      let barrier = AsyncBarrier::new(2);
      let other = barrier.clone();

      let first = barrier.wait();
      let second = other.wait();

      join!(first, second);
    });
  }
}
