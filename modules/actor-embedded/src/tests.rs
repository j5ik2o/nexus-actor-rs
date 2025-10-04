#[cfg(feature = "embassy")]
mod embassy {
  extern crate alloc;
  extern crate std;

  use alloc::{boxed::Box, sync::Arc};
  use core::sync::atomic::{AtomicUsize, Ordering};
  use core::time::Duration;
  use embassy_executor::raw::Executor;
  use nexus_actor_core_rs::runtime::{CoreScheduledTask, CoreScheduler};

  use crate::spawn::{EmbassyCoreSpawner, EmbassyScheduler, EmbassyTaskSlot};

  use std::cell::RefCell;
  use std::sync::{Mutex, OnceLock};

  std::thread_local! {
    static CS_DEPTH: RefCell<usize> = RefCell::new(0);
    static CS_GUARD: RefCell<Option<std::sync::MutexGuard<'static, ()>>> = RefCell::new(None);
  }

  static CS_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
  static NEXT_NOW: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

  #[allow(improper_ctypes_definitions)]
  #[no_mangle]
  extern "C" fn _critical_section_1_0_acquire() {
    let mutex = CS_MUTEX.get_or_init(|| Mutex::new(()));
    CS_DEPTH.with(|depth| {
      let mut depth_mut = depth.borrow_mut();
      if *depth_mut == 0 {
        let guard = mutex.lock().expect("critical-section mutex poisoned");
        CS_GUARD.with(|cell| {
          *cell.borrow_mut() = Some(guard);
        });
      }
      *depth_mut += 1;
    });
  }

  #[allow(improper_ctypes_definitions)]
  #[no_mangle]
  extern "C" fn _critical_section_1_0_release(_: ()) {
    CS_DEPTH.with(|depth| {
      let mut depth_mut = depth.borrow_mut();
      if *depth_mut == 0 {
        return;
      }
      *depth_mut -= 1;
      if *depth_mut == 0 {
        CS_GUARD.with(|cell| {
          cell.borrow_mut().take();
        });
      }
    });
  }

  #[no_mangle]
  extern "C" fn _embassy_time_now() -> u64 {
    NEXT_NOW.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
  }

  #[no_mangle]
  extern "C" fn _embassy_time_schedule_wake(_: u64) {}

  #[export_name = "__pender"]
  fn __pender(_context: *mut ()) {}

  static TASK_SLOTS: [EmbassyTaskSlot; 4] = [const { EmbassyTaskSlot::new() }; 4];

  fn build_scheduler() -> (EmbassyScheduler, &'static Executor) {
    let executor = Box::leak(Box::new(Executor::new(core::ptr::null_mut())));
    let send_spawner = executor.spawner().make_send();
    let core_spawner = Arc::new(EmbassyCoreSpawner::new(send_spawner, &TASK_SLOTS));
    (EmbassyScheduler::new(core_spawner), executor)
  }

  fn make_counter_task(counter: Arc<AtomicUsize>) -> CoreScheduledTask {
    Arc::new(move || {
      let counter_clone = counter.clone();
      Box::pin(async move {
        counter_clone.fetch_add(1, Ordering::Relaxed);
      })
    })
  }

  #[test]
  fn test_embassy_scheduler_schedule_once_executes_immediately() {
    let (scheduler, executor) = build_scheduler();
    let counter = Arc::new(AtomicUsize::new(0));
    let handle = scheduler.schedule_once(Duration::ZERO, make_counter_task(counter.clone()));

    assert!(handle.is_active());

    unsafe { executor.poll() };

    assert_eq!(counter.load(Ordering::Relaxed), 1);
    assert!(!handle.is_active());
  }

  #[test]
  fn test_embassy_scheduler_drain_cancels_pending_tasks() {
    let (scheduler, executor) = build_scheduler();
    let counter = Arc::new(AtomicUsize::new(0));
    let handle = scheduler.schedule_once(Duration::from_millis(10), make_counter_task(counter.clone()));

    assert!(handle.is_active());
    scheduler.drain();

    unsafe { executor.poll() };

    assert_eq!(counter.load(Ordering::Relaxed), 0);
    assert!(!handle.is_active());
  }

  #[test]
  fn test_embassy_scheduler_schedule_repeated_zero_interval_completes_once() {
    let (scheduler, executor) = build_scheduler();
    let counter = Arc::new(AtomicUsize::new(0));
    let handle = scheduler.schedule_repeated(Duration::ZERO, Duration::ZERO, make_counter_task(counter.clone()));

    unsafe { executor.poll() };

    assert_eq!(counter.load(Ordering::Relaxed), 1);
    assert!(!handle.is_active());
  }
}
