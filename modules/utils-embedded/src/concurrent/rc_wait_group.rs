use alloc::boxed::Box;
use alloc::rc::Rc;

use core::cell::RefCell;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::signal::Signal;
use nexus_utils_core_rs::{async_trait, WaitGroup as CoreWaitGroup, WaitGroupBackend};

/// `Rc` ベースの待機グループ実装のバックエンド。
///
/// `no_std` 環境で、複数のタスクの完了を追跡し、すべてが完了するまで待機する機構を提供します。
/// カウントを動的に増やしたり減らしたりでき、カウントが 0 になると待機しているタスクが解放されます。
///
/// # 特徴
///
/// - `Rc` による参照カウント（シングルスレッド専用）
/// - Embassy の `Signal` による非同期シグナリング
/// - 動的なカウント管理（`add` でカウント追加、`done` でカウント減少）
///
/// # 使用例
///
/// ```ignore
/// let wg = WaitGroup::new();
/// wg.add(2);
///
/// let clone = wg.clone();
/// async move {
///   // 作業1
///   clone.done();
///   // 作業2
///   clone.done();
/// };
///
/// // すべてのタスクが完了するまで待機
/// wg.wait().await;
/// ```
#[derive(Clone)]
pub struct RcWaitGroupBackend {
  count: Rc<RefCell<usize>>,
  signal: Rc<Signal<NoopRawMutex, ()>>,
}

#[async_trait(?Send)]
impl WaitGroupBackend for RcWaitGroupBackend {
  /// カウント 0 で新しい待機グループバックエンドを作成します。
  fn new() -> Self {
    Self::with_count(0)
  }

  /// 指定されたカウントで新しい待機グループバックエンドを作成します。
  ///
  /// # Arguments
  ///
  /// * `count` - 初期カウント値
  fn with_count(count: usize) -> Self {
    Self {
      count: Rc::new(RefCell::new(count)),
      signal: Rc::new(Signal::new()),
    }
  }

  /// カウントを指定された数だけ増やします。
  ///
  /// # Arguments
  ///
  /// * `n` - 追加するカウント数
  fn add(&self, n: usize) {
    *self.count.borrow_mut() += n;
  }

  /// カウントを 1 減らします。
  ///
  /// カウントが 0 になると、待機中のすべてのタスクがシグナルを受け取り解放されます。
  ///
  /// # Panics
  ///
  /// カウントが既に 0 の状態で呼び出された場合はパニックします。
  fn done(&self) {
    let mut count = self.count.borrow_mut();
    assert!(*count > 0, "WaitGroup::done called more times than add");
    *count -= 1;
    if *count == 0 {
      self.signal.signal(());
    }
  }

  /// カウントが 0 になるまで待機します。
  ///
  /// カウントが既に 0 の場合は即座に返ります。
  async fn wait(&self) {
    let count = self.count.clone();
    let signal = self.signal.clone();
    loop {
      if *count.borrow() == 0 {
        return;
      }
      signal.wait().await;
    }
  }
}

/// `Rc` ベースの待機グループの型エイリアス。
///
/// `no_std` 環境で使用できる待機グループ実装です。
/// 複数のタスクの完了を追跡し、すべてが完了するまで待機する機能を提供します。
pub type WaitGroup = CoreWaitGroup<RcWaitGroupBackend>;

#[cfg(test)]
mod tests {
  use super::WaitGroup;
  use futures::executor::block_on;
  use futures::join;

  #[test]
  fn wait_group_completes() {
    block_on(async {
      let wg = WaitGroup::new();
      wg.add(2);
      let clone = wg.clone();
      let worker = async move {
        clone.done();
        clone.done();
      };
      join!(worker, wg.wait());
    });
  }
}
