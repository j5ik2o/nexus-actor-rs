use alloc::boxed::Box;
use alloc::rc::Rc;

use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_sync::signal::Signal;
use nexus_utils_core_rs::{async_trait, CountDownLatch as CoreCountDownLatch, CountDownLatchBackend};

/// `Rc` ベースのカウントダウンラッチ実装のバックエンド。
///
/// `no_std` 環境で、複数のタスクが完了するまで待機する同期機構を提供します。
/// カウントが指定された値から 0 にデクリメントされると、待機中のすべてのタスクが解放されます。
///
/// # 特徴
///
/// - `Rc` による参照カウント（シングルスレッド専用）
/// - Embassy の `Mutex` と `Signal` による非同期同期
/// - 一方向カウントダウン（カウントは減少のみ）
///
/// # 使用例
///
/// ```ignore
/// let latch = CountDownLatch::new(2);
/// let clone = latch.clone();
///
/// // ワーカータスク
/// async move {
///   // 作業を実行
///   clone.count_down().await;
///   // さらに作業を実行
///   clone.count_down().await;
/// };
///
/// // カウントが 0 になるまで待機
/// latch.wait().await;
/// ```
#[derive(Clone)]
pub struct RcCountDownLatchBackend {
  count: Rc<Mutex<NoopRawMutex, usize>>,
  signal: Rc<Signal<NoopRawMutex, ()>>,
}

#[async_trait(?Send)]
impl CountDownLatchBackend for RcCountDownLatchBackend {
  /// 指定されたカウントで新しいラッチバックエンドを作成します。
  ///
  /// # 引数
  ///
  /// * `count` - 初期カウント値（0も許可）
  fn new(count: usize) -> Self {
    Self {
      count: Rc::new(Mutex::new(count)),
      signal: Rc::new(Signal::new()),
    }
  }

  /// カウントを 1 減らします。
  ///
  /// カウントが 0 になると、待機中のすべてのタスクがシグナルを受け取り解放されます。
  ///
  /// # パニック
  ///
  /// カウントが既に 0 の状態で呼び出された場合はパニックします。
  async fn count_down(&self) {
    let count = self.count.clone();
    let signal = self.signal.clone();
    let mut guard = count.lock().await;
    assert!(*guard > 0, "CountDownLatch::count_down called too many times");
    *guard -= 1;
    if *guard == 0 {
      signal.signal(());
    }
  }

  /// カウントが 0 になるまで待機します。
  ///
  /// カウントが既に 0 の場合は即座に返ります。
  async fn wait(&self) {
    let count = self.count.clone();
    let signal = self.signal.clone();
    loop {
      {
        let guard = count.lock().await;
        if *guard == 0 {
          return;
        }
      }
      signal.wait().await;
    }
  }
}

/// `Rc` ベースのカウントダウンラッチの型エイリアス。
///
/// `no_std` 環境で使用できるカウントダウンラッチ実装です。
/// カウントが 0 になるまで複数のタスクの完了を待機する機能を提供します。
pub type CountDownLatch = CoreCountDownLatch<RcCountDownLatchBackend>;

#[cfg(test)]
mod tests {
  use super::CountDownLatch;
  use futures::executor::block_on;
  use futures::join;

  #[test]
  fn latch_reaches_zero() {
    block_on(async {
      let latch = CountDownLatch::new(2);
      let clone = latch.clone();

      let wait_fut = latch.wait();
      let worker = async move {
        clone.count_down().await;
        clone.count_down().await;
      };

      join!(worker, wait_fut);
    });
  }
}
