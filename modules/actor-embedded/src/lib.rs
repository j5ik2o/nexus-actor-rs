//! Actor system implementation for embedded environments.
//!
//! This crate provides implementations for running actor systems in `no_std` environments.
//! Supports local mailboxes, Arc-based mailboxes, Embassy integration, and more.

#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::missing_errors_doc)]
#![deny(clippy::missing_panics_doc)]
#![deny(clippy::missing_safety_doc)]
#![deny(clippy::redundant_clone)]
#![deny(clippy::redundant_field_names)]
#![deny(clippy::redundant_pattern)]
#![deny(clippy::redundant_static_lifetimes)]
#![deny(clippy::unnecessary_to_owned)]
#![deny(clippy::unnecessary_struct_initialization)]
#![deny(clippy::needless_borrow)]
#![deny(clippy::needless_pass_by_value)]
#![deny(clippy::manual_ok_or)]
#![deny(clippy::manual_map)]
#![deny(clippy::manual_let_else)]
#![deny(clippy::manual_strip)]
#![deny(clippy::unused_async)]
#![deny(clippy::unused_self)]
#![deny(clippy::unnecessary_wraps)]
#![deny(clippy::unreachable)]
#![deny(clippy::empty_enum)]
#![deny(clippy::no_effect)]
#![deny(clippy::drop_copy)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::todo)]
#![deny(clippy::unimplemented)]
#![deny(clippy::print_stdout)]
#![deny(clippy::dbg_macro)]
#![deny(clippy::missing_const_for_fn)]
#![deny(clippy::must_use_candidate)]
#![deny(clippy::trivially_copy_pass_by_ref)]
#![deny(clippy::clone_on_copy)]
#![deny(clippy::len_without_is_empty)]
#![deny(clippy::wrong_self_convention)]
#![deny(clippy::wrong_pub_self_convention)]
#![deny(clippy::from_over_into)]
#![deny(clippy::eq_op)]
#![deny(clippy::bool_comparison)]
#![deny(clippy::needless_bool)]
#![deny(clippy::match_like_matches_macro)]
#![deny(clippy::manual_assert)]
#![deny(clippy::naive_bytecount)]
#![deny(clippy::if_same_then_else)]
#![deny(clippy::cmp_null)]
#![no_std]

extern crate alloc;

#[cfg(feature = "embedded_arc")]
mod arc_mailbox;
#[cfg(feature = "embedded_arc")]
mod arc_priority_mailbox;
#[cfg(feature = "embassy_executor")]
mod embassy_dispatcher;
mod local_mailbox;
mod runtime_driver;
mod spawn;
mod timer;

#[cfg(feature = "embedded_arc")]
pub use arc_mailbox::{ArcMailbox, ArcMailboxFactory, ArcMailboxSender};
#[cfg(feature = "embedded_arc")]
pub use arc_priority_mailbox::{ArcPriorityMailbox, ArcPriorityMailboxFactory, ArcPriorityMailboxSender};
#[cfg(feature = "embassy_executor")]
pub use embassy_dispatcher::spawn_embassy_dispatcher;
pub use local_mailbox::{LocalMailbox, LocalMailboxFactory, LocalMailboxSender};
#[cfg(feature = "embedded_arc")]
pub use nexus_utils_embedded_rs::{ArcCsStateCell, ArcLocalStateCell, ArcShared, ArcStateCell};
#[cfg(feature = "embedded_rc")]
pub use nexus_utils_embedded_rs::{RcShared, RcStateCell};
pub use runtime_driver::EmbeddedFailureEventHub;
pub use spawn::ImmediateSpawner;
pub use timer::ImmediateTimer;

/// Prelude that re-exports commonly used types in embedded environments.
pub mod prelude {
  #[cfg(feature = "embedded_arc")]
  pub use super::{
    ArcCsStateCell, ArcLocalStateCell, ArcMailbox, ArcMailboxFactory, ArcMailboxSender, ArcPriorityMailbox,
    ArcPriorityMailboxFactory, ArcPriorityMailboxSender, ArcShared, ArcStateCell,
  };
  pub use super::{ImmediateSpawner, ImmediateTimer, LocalMailbox, LocalMailboxFactory, LocalMailboxSender};
  #[cfg(feature = "embedded_rc")]
  pub use super::{RcShared, RcStateCell};
}

#[cfg(test)]
mod tests {
  extern crate std;

  use super::LocalMailboxFactory;
  use alloc::rc::Rc;
  use alloc::vec::Vec;
  use core::cell::RefCell;
  use core::future::Future;
  use core::pin::Pin;
  use core::task::{Context, Poll};
  use futures::task::{waker, ArcWake};
  use nexus_actor_core_rs::{ActorSystem, MailboxOptions, Props};
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
    let factory = LocalMailboxFactory::default();
    let mut system: ActorSystem<u32, _> = ActorSystem::new(factory);

    let log: Rc<RefCell<Vec<u32>>> = Rc::new(RefCell::new(Vec::new()));
    let log_clone = log.clone();

    let props = Props::new(MailboxOptions::default(), move |_, msg: u32| {
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
