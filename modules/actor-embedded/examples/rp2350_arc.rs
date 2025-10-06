#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_std)]
#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_main)]

#[cfg(all(target_arch = "arm", target_os = "none"))]
extern crate alloc;

#[cfg(all(target_arch = "arm", target_os = "none"))]
use alloc_cortex_m::CortexMHeap;
#[cfg(all(target_arch = "arm", target_os = "none"))]
use core::future::Future;
#[cfg(all(target_arch = "arm", target_os = "none"))]
use core::pin::Pin;
#[cfg(all(target_arch = "arm", target_os = "none"))]
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
#[cfg(all(target_arch = "arm", target_os = "none"))]
use cortex_m::{asm, interrupt};
#[cfg(all(target_arch = "arm", target_os = "none"))]
use cortex_m_rt::entry;
#[cfg(all(target_arch = "arm", target_os = "none"))]
use panic_halt as _;

#[cfg(all(target_arch = "arm", target_os = "none"))]
use nexus_actor_core_rs::{actor_loop, Mailbox};
#[cfg(all(target_arch = "arm", target_os = "none"))]
use nexus_actor_embedded_rs::prelude::{ImmediateTimer, LocalMailbox};
#[cfg(all(target_arch = "arm", target_os = "none"))]
use nexus_actor_embedded_rs::ArcStateCell;

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[global_allocator]
static ALLOCATOR: CortexMHeap = CortexMHeap::empty();

#[cfg(all(target_arch = "arm", target_os = "none"))]
const HEAP_SIZE: usize = 16 * 1024;
#[cfg(all(target_arch = "arm", target_os = "none"))]
static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[entry]
fn main() -> ! {
  let heap_start = unsafe { core::ptr::addr_of_mut!(HEAP) as usize };
  interrupt::free(|_| unsafe {
    ALLOCATOR.init(heap_start, HEAP_SIZE);
  });

  let mailbox = LocalMailbox::new();
  Mailbox::try_send(&mailbox, 2u32).ok();

  let state = ArcStateCell::new(0u32);
  let state_clone = state.clone();

  let mut future = actor_loop(&mailbox, &ImmediateTimer, move |message| {
    let mut guard = state_clone.borrow_mut();
    *guard = guard.wrapping_add(message);
  });

  let mut future = unsafe { Pin::new_unchecked(&mut future) };
  let waker = unsafe { Waker::from_raw(dummy_raw_waker()) };
  let mut cx = Context::from_waker(&waker);

  loop {
    match future.as_mut().poll(&mut cx) {
      Poll::Ready(_) => break,
      Poll::Pending => {
        if *state.borrow() >= 2 {
          break;
        }
      }
    }
  }

  asm::bkpt();
  loop {}
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn dummy_raw_waker() -> RawWaker {
  fn no_op(_: *const ()) {}
  fn clone(_: *const ()) -> RawWaker {
    dummy_raw_waker()
  }
  const VTABLE: RawWakerVTable = RawWakerVTable::new(clone, no_op, no_op, no_op);
  RawWaker::new(core::ptr::null(), &VTABLE)
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn main() {
  eprintln!("rp2350_arc example targets Cortex-M33 (e.g., thumbv8m.main-none-eabihf). Build with --target thumbv8m.main-none-eabihf --no-default-features --features alloc,embedded_arc.");
}
