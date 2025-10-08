#![no_std]

extern crate alloc;

#[cfg(feature = "embedded_arc")]
mod arc_mailbox;
#[cfg(feature = "embedded_arc")]
mod arc_priority_mailbox;
#[cfg(feature = "embassy_executor")]
mod embassy_dispatcher;
mod local_mailbox;
mod spawn;
mod timer;

#[cfg(feature = "embedded_arc")]
pub use arc_mailbox::{ArcMailbox, ArcMailboxRuntime, ArcMailboxSender};
#[cfg(feature = "embedded_arc")]
pub use arc_priority_mailbox::{ArcPriorityMailbox, ArcPriorityMailboxRuntime, ArcPriorityMailboxSender};
#[cfg(feature = "embassy_executor")]
pub use embassy_dispatcher::spawn_embassy_dispatcher;
pub use local_mailbox::{LocalMailbox, LocalMailboxRuntime, LocalMailboxSender};
#[cfg(feature = "embedded_arc")]
pub use nexus_utils_embedded_rs::{ArcCsStateCell, ArcLocalStateCell, ArcShared, ArcStateCell};
#[cfg(feature = "embedded_rc")]
pub use nexus_utils_embedded_rs::{RcShared, RcStateCell};
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
  use core::task::{Context, Poll};
  use futures::task::{waker, ArcWake};
  use nexus_actor_core_rs::{MailboxOptions, TypedActorSystem, TypedProps};
  use std::sync::{Arc, Condvar, Mutex};

  fn block_on<F: Future>(mut future: F) -> F::Output {
    struct WaitCell {
      state: Mutex<bool>,
      cvar: Condvar,
    }

    impl ArcWake for WaitCell {
      fn wake_by_ref(arc_self: &Arc<Self>) {
        let mut ready = arc_self.state.lock().unwrap();
        *ready = true;
        arc_self.cvar.notify_one();
      }
    }

    let cell = Arc::new(WaitCell {
      state: Mutex::new(false),
      cvar: Condvar::new(),
    });
    let waker = waker(cell.clone());
    let mut cx = Context::from_waker(&waker);
    // Safety: we never move `future` after pinning.
    let mut pinned = unsafe { Pin::new_unchecked(&mut future) };
    loop {
      match pinned.as_mut().poll(&mut cx) {
        Poll::Ready(output) => break output,
        Poll::Pending => {
          let mut ready = cell.state.lock().unwrap();
          while !*ready {
            ready = cell.cvar.wait(ready).unwrap();
          }
          *ready = false;
        }
      }
    }
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
