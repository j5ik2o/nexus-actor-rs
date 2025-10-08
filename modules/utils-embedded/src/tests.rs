#[cfg(feature = "arc")]
mod critical_section {
  use core::sync::atomic::{AtomicBool, Ordering};
  use critical_section::{Impl, RawRestoreState};

  struct TestCriticalSection;

  static LOCK: AtomicBool = AtomicBool::new(false);
  static INIT: AtomicBool = AtomicBool::new(false);

  unsafe impl Impl for TestCriticalSection {
    unsafe fn acquire() -> RawRestoreState {
      while LOCK
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
      {}
      ()
    }

    unsafe fn release(_: RawRestoreState) {
      LOCK.store(false, Ordering::SeqCst);
    }
  }

  pub(crate) fn init_arc_critical_section() {
    if INIT
      .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
      .is_ok()
    {
      critical_section::set_impl!(TestCriticalSection);
    }
  }
}

#[cfg(feature = "arc")]
pub(crate) use critical_section::init_arc_critical_section;
