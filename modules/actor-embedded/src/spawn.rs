use core::future::Future;

use nexus_actor_core_rs::Spawn;

/// Futureを即座にドロップするスポーナー。
///
/// 組み込み環境向けに、タスクを実際には実行せずにドロップするだけの実装です。
pub struct ImmediateSpawner;

impl Spawn for ImmediateSpawner {
  fn spawn(&self, _fut: impl Future<Output = ()> + 'static) {}
}

#[cfg(test)]
mod tests {
  extern crate std;

  use super::*;
  use std::sync::atomic::{AtomicBool, Ordering};
  use std::sync::Arc;

  #[test]
  fn immediate_spawner_drops_future_without_polling() {
    let spawner = ImmediateSpawner;
    let polled = Arc::new(AtomicBool::new(false));
    let flag = polled.clone();

    spawner.spawn(async move {
      flag.store(true, Ordering::SeqCst);
    });

    assert!(
      !polled.load(Ordering::SeqCst),
      "future should not be polled by ImmediateSpawner"
    );
  }
}
