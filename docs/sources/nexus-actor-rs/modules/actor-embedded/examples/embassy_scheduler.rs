#[cfg(feature = "embassy")]
use core::sync::atomic::{AtomicUsize, Ordering};
#[cfg(feature = "embassy")]
use core::time::Duration;
#[cfg(feature = "embassy")]
extern crate alloc;

#[cfg(feature = "embassy")]
use alloc::{boxed::Box, sync::Arc};

#[cfg(feature = "embassy")]
use embassy_executor::raw::Executor;
#[cfg(feature = "embassy")]
use nexus_actor_core_rs::runtime::{CoreScheduledTask, CoreScheduler};
#[cfg(feature = "embassy")]
use nexus_actor_embedded_rs::spawn::{EmbassyCoreSpawner, EmbassyScheduler, EmbassyTaskSlot};

#[cfg(feature = "embassy")]
use std::cell::RefCell;
#[cfg(feature = "embassy")]
use std::sync::atomic::{AtomicU64, Ordering as StdOrdering};
#[cfg(feature = "embassy")]
use std::sync::{Mutex, OnceLock};

#[cfg(feature = "embassy")]
std::thread_local! {
  static CS_DEPTH: RefCell<usize> = RefCell::new(0);
  static CS_GUARD: RefCell<Option<std::sync::MutexGuard<'static, ()>>> = RefCell::new(None);
}

#[cfg(feature = "embassy")]
static CS_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
#[cfg(feature = "embassy")]
static NEXT_NOW: AtomicU64 = AtomicU64::new(0);

#[cfg(feature = "embassy")]
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

#[cfg(feature = "embassy")]
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

#[cfg(feature = "embassy")]
#[no_mangle]
extern "C" fn _embassy_time_now() -> u64 {
  NEXT_NOW.fetch_add(1, StdOrdering::Relaxed)
}

#[cfg(feature = "embassy")]
#[no_mangle]
extern "C" fn _embassy_time_schedule_wake(_: u64) {}

#[cfg(feature = "embassy")]
#[export_name = "__pender"]
fn __pender(_context: *mut ()) {}

#[cfg(feature = "embassy")]
static TASK_SLOTS: [EmbassyTaskSlot; 4] = [const { EmbassyTaskSlot::new() }; 4];

#[cfg(feature = "embassy")]
fn main() {
  // Executor must live for the entire program; leak to obtain 'static lifetime.
  let executor = Box::leak(Box::new(Executor::new(core::ptr::null_mut())));
  let send_spawner = executor.spawner().make_send();

  let core_spawner = Arc::new(EmbassyCoreSpawner::new(send_spawner, &TASK_SLOTS));
  let scheduler = EmbassyScheduler::new(core_spawner);

  let counter = Arc::new(AtomicUsize::new(0));
  let task_counter = counter.clone();
  let task: CoreScheduledTask = Arc::new(move || {
    let task_counter = task_counter.clone();
    Box::pin(async move {
      task_counter.fetch_add(1, Ordering::Relaxed);
    })
  });

  // Delay を 0 にして即時実行させ、Embassy executor を1回だけポーリングする。
  scheduler.schedule_once(Duration::ZERO, task);

  unsafe { executor.poll() };

  assert_eq!(counter.load(Ordering::Relaxed), 1);
}

#[cfg(not(feature = "embassy"))]
fn main() {
  eprintln!("このサンプルを実行するには `--features embassy` を有効にしてください。");
}
