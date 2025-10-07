#![no_std]

extern crate alloc;

#[cfg(feature = "embedded_arc")]
mod arc_mailbox;
#[cfg(feature = "embedded_arc")]
mod arc_priority_mailbox;
mod local_mailbox;
mod spawn;
mod timer;

#[cfg(feature = "embedded_arc")]
pub use arc_mailbox::{ArcMailbox, ArcMailboxRuntime, ArcMailboxSender};
#[cfg(feature = "embedded_arc")]
pub use arc_priority_mailbox::{ArcPriorityMailbox, ArcPriorityMailboxRuntime, ArcPriorityMailboxSender};
pub use local_mailbox::{LocalMailbox, LocalMailboxRuntime, LocalMailboxSender};
#[cfg(feature = "embedded_arc")]
pub use nexus_utils_embedded_rs::sync::{ArcCsStateCell, ArcLocalStateCell, ArcShared, ArcStateCell};
#[cfg(feature = "embedded_rc")]
pub use nexus_utils_embedded_rs::sync::{RcShared, RcStateCell};
pub use spawn::ImmediateSpawner;
pub use timer::ImmediateTimer;

pub mod prelude {
  #[cfg(feature = "embedded_arc")]
  pub use super::{
    ArcCsStateCell, ArcLocalStateCell, ArcMailbox, ArcMailboxRuntime, ArcMailboxSender, ArcPriorityMailbox,
    ArcPriorityMailboxRuntime, ArcPriorityMailboxSender, ArcShared, ArcStateCell,
  };
  pub use super::{ImmediateSpawner, ImmediateTimer, LocalMailbox, LocalMailboxRuntime, LocalMailboxSender};
  #[cfg(feature = "embedded_rc")]
  pub use super::{RcShared, RcStateCell};
}

#[cfg(test)]
mod tests {
  extern crate std;

  use super::LocalMailboxRuntime;
  use alloc::rc::Rc;
  use alloc::vec::Vec;
  use core::cell::RefCell;
  use core::future::Future;
  use core::pin::Pin;
  use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
  use nexus_actor_core_rs::{MailboxOptions, TypedActorSystem, TypedProps};

  fn block_on<F: Future>(mut future: F) -> F::Output {
    let waker = unsafe { Waker::from_raw(noop_raw_waker()) };
    let mut cx = Context::from_waker(&waker);
    // Safety: we never move `future` after pinning.
    let mut pinned = unsafe { Pin::new_unchecked(&mut future) };
    loop {
      match pinned.as_mut().poll(&mut cx) {
        Poll::Ready(output) => break output,
        Poll::Pending => core::hint::spin_loop(),
      }
    }
  }

  unsafe fn noop_raw_waker() -> RawWaker {
    fn clone(_: *const ()) -> RawWaker {
      unsafe { noop_raw_waker() }
    }
    fn wake(_: *const ()) {}
    fn wake_by_ref(_: *const ()) {}
    fn drop(_: *const ()) {}

    RawWaker::new(core::ptr::null(), &RawWakerVTable::new(clone, wake, wake_by_ref, drop))
  }

  #[test]
  fn typed_actor_system_dispatch_next_processes_message() {
    let runtime = LocalMailboxRuntime::default();
    let mut system: TypedActorSystem<u32, _> = TypedActorSystem::new(runtime);

    let log: Rc<RefCell<Vec<u32>>> = Rc::new(RefCell::new(Vec::new()));
    let log_clone = log.clone();

    let props = TypedProps::new(MailboxOptions::default(), move |_, msg: u32| {
      log_clone.borrow_mut().push(msg);
    });

    let mut root = system.root_context();
    let actor_ref = root.spawn(props).expect("spawn typed actor");

    actor_ref.tell(21).expect("tell message");

    block_on(async {
      root.dispatch_next().await.expect("dispatch next");
    });

    assert_eq!(log.borrow().as_slice(), &[21]);
  }
}
