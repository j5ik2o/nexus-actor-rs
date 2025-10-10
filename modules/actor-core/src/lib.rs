//! nexus-actor-rs core library
//!
//! Core module of the actor model library implemented in Rust.
//! Provides type-safe message passing, supervisor hierarchies,
//! and Akka/Pekko Typed-style Behavior API.
//!
//! # Key Features
//! - Typed actor references (`ActorRef<U, R>`)
//! - Behavior DSL (Akka Typed-style)
//! - Supervisor strategies
//! - Ask pattern (Request-Response)
//! - Mailboxes and dispatchers
//! - Event stream
//!
//! # Example Usage
//! ```ignore
//! use nexus_actor_core_rs::*;
//!
//! let behavior = Behaviors::receive(|ctx, msg: String| {
//!     println!("Received: {}", msg);
//!     Behaviors::same()
//! });
//! ```

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
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::result_large_err)]

#[cfg(feature = "alloc")]
extern crate alloc;

use core::time::Duration;
use nexus_utils_core_rs::QueueError;

mod api;
#[cfg(feature = "alloc")]
mod extensions;
mod runtime;
mod shared;

pub use api::*;
#[cfg(feature = "alloc")]
pub use extensions::{next_extension_id, Extension, ExtensionId, Extensions};
#[cfg(feature = "alloc")]
pub use extensions::{serializer_extension_id, SerializerRegistryExtension};
pub use runtime::mailbox::traits::{SingleThread, ThreadSafe};
pub use runtime::mailbox::{PriorityEnvelope, SystemMessage};
pub use runtime::message::{
  discard_metadata, store_metadata, take_metadata, DynMessage, MetadataKey, MetadataStorageMode,
};
pub use runtime::scheduler::{ReceiveTimeoutScheduler, ReceiveTimeoutSchedulerFactory};
pub use shared::{FailureEventHandlerShared, FailureEventListenerShared, MapSystemShared, ReceiveTimeoutFactoryShared};

/// Marker trait capturing the synchronization guarantees required by runtime-dependent types.
#[cfg(target_has_atomic = "ptr")]
pub trait RuntimeBound: Send + Sync {}

#[cfg(target_has_atomic = "ptr")]
impl<T: Send + Sync> RuntimeBound for T {}

#[cfg(not(target_has_atomic = "ptr"))]
/// Marker trait for single-threaded targets without atomic pointer support.
pub trait RuntimeBound {}

#[cfg(not(target_has_atomic = "ptr"))]
impl<T> RuntimeBound for T {}

/// Function type alias for converting system messages to message type.
#[cfg(target_has_atomic = "ptr")]
pub type MapSystemFn<M> = dyn Fn(SystemMessage) -> M + Send + Sync;

/// Function type alias for converting system messages on non-atomic targets.
#[cfg(not(target_has_atomic = "ptr"))]
pub type MapSystemFn<M> = dyn Fn(SystemMessage) -> M;

/// Minimal actor loop implementation.
///
/// Receives messages and passes them to the handler for processing.
/// Reference implementation shared by both std and embedded runtimes.
///
/// # Arguments
/// * `mailbox` - Mailbox to receive messages from
/// * `timer` - Timer used for waiting
/// * `handler` - Handler function to process messages
pub async fn actor_loop<M, MB, T, F>(mailbox: &MB, timer: &T, mut handler: F)
where
  MB: Mailbox<M>,
  T: Timer,
  F: FnMut(M), {
  loop {
    match mailbox.recv().await {
      Ok(message) => handler(message),
      Err(QueueError::Disconnected) => break,
      Err(QueueError::Closed(message)) => handler(message),
      Err(_) => break,
    }
    timer.sleep(Duration::from_millis(0)).await;
  }
}

#[cfg(test)]
mod tests {
  extern crate alloc;

  use super::*;
  use alloc::rc::Rc;
  use core::cell::{Ref, RefCell, RefMut};
  use core::fmt;
  use core::future::Future;
  use core::pin::Pin;
  use core::ptr;
  use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
  use nexus_utils_core_rs::{MpscBuffer, MpscHandle, MpscQueue, QueueError, RingBufferBackend, Shared, StateCell};

  struct TestStateCell<T>(Rc<RefCell<T>>);

  impl<T> TestStateCell<T> {
    fn new(value: T) -> Self {
      Self(Rc::new(RefCell::new(value)))
    }
  }

  impl<T> Clone for TestStateCell<T> {
    fn clone(&self) -> Self {
      Self(self.0.clone())
    }
  }

  impl<T> StateCell<T> for TestStateCell<T> {
    type Ref<'a>
      = Ref<'a, T>
    where
      Self: 'a,
      T: 'a;
    type RefMut<'a>
      = RefMut<'a, T>
    where
      Self: 'a,
      T: 'a;

    fn new(value: T) -> Self
    where
      Self: Sized, {
      TestStateCell::new(value)
    }

    fn borrow(&self) -> Self::Ref<'_> {
      self.0.borrow()
    }

    fn borrow_mut(&self) -> Self::RefMut<'_> {
      self.0.borrow_mut()
    }
  }

  #[derive(Clone, Default)]
  struct TestSignal {
    state: Rc<RefCell<SignalState>>,
  }

  #[derive(Default)]
  struct SignalState {
    notified: bool,
    waker: Option<Waker>,
  }

  impl MailboxSignal for TestSignal {
    type WaitFuture<'a>
      = TestSignalWait
    where
      Self: 'a;

    fn notify(&self) {
      let mut state = self.state.borrow_mut();
      state.notified = true;
      if let Some(waker) = state.waker.take() {
        waker.wake();
      }
    }

    fn wait(&self) -> Self::WaitFuture<'_> {
      TestSignalWait { signal: self.clone() }
    }
  }

  struct TestSignalWait {
    signal: TestSignal,
  }

  impl Future for TestSignalWait {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
      let mut state = self.signal.state.borrow_mut();
      if state.notified {
        state.notified = false;
        Poll::Ready(())
      } else {
        state.waker = Some(cx.waker().clone());
        Poll::Pending
      }
    }
  }

  struct RcBackendHandle<T>(Rc<RingBufferBackend<RefCell<MpscBuffer<T>>>>);

  impl<T> RcBackendHandle<T> {
    fn new(capacity: Option<usize>) -> Self {
      let buffer = RefCell::new(MpscBuffer::new(capacity));
      let backend = RingBufferBackend::new(buffer);
      Self(Rc::new(backend))
    }
  }

  impl<T> Clone for RcBackendHandle<T> {
    fn clone(&self) -> Self {
      Self(self.0.clone())
    }
  }

  impl<T> fmt::Debug for RcBackendHandle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      f.debug_struct("RcBackendHandle").finish()
    }
  }

  impl<T> core::ops::Deref for RcBackendHandle<T> {
    type Target = RingBufferBackend<RefCell<MpscBuffer<T>>>;

    fn deref(&self) -> &Self::Target {
      &self.0
    }
  }

  impl<T> Shared<RingBufferBackend<RefCell<MpscBuffer<T>>>> for RcBackendHandle<T> {}

  impl<T> MpscHandle<T> for RcBackendHandle<T> {
    type Backend = RingBufferBackend<RefCell<MpscBuffer<T>>>;

    fn backend(&self) -> &Self::Backend {
      &self.0
    }
  }

  struct TestTimer;

  impl Timer for TestTimer {
    type SleepFuture<'a>
      = core::future::Ready<()>
    where
      Self: 'a;

    fn sleep(&self, _duration: Duration) -> Self::SleepFuture<'_> {
      core::future::ready(())
    }
  }

  #[test]
  fn actor_loop_updates_state_cell_with_message() {
    type TestQueue<T> = MpscQueue<RcBackendHandle<T>, T>;

    let queue: TestQueue<u32> = MpscQueue::new(RcBackendHandle::new(None));
    let mailbox = QueueMailbox::new(queue, TestSignal::default());
    mailbox.try_send(3_u32).unwrap();
    let timer = TestTimer;
    let state = TestStateCell::new(0_u32);
    let state_for_handler = state.clone();

    let mut future = actor_loop(&mailbox, &timer, move |message| {
      let mut value = state_for_handler.borrow_mut();
      *value += message;
    });

    match poll_once(&mut future) {
      Poll::Pending => {}
      Poll::Ready(_) => panic!("actor_loop should not complete"),
    }

    assert_eq!(*state.borrow(), 3);

    assert!(matches!(poll_once(&mut future), Poll::Pending));
  }

  #[test]
  fn queue_mailbox_handles_close_and_disconnect() {
    type TestQueue<T> = MpscQueue<RcBackendHandle<T>, T>;

    let queue: TestQueue<u32> = MpscQueue::new(RcBackendHandle::new(None));
    let mailbox = QueueMailbox::new(queue, TestSignal::default());

    let mut recv_future = mailbox.recv();
    assert!(matches!(poll_once(&mut recv_future), Poll::Pending));

    mailbox.try_send(42).unwrap();
    assert_eq!(poll_once(&mut recv_future), Poll::Ready(Ok(42)));

    mailbox.close();
    assert!(matches!(
      mailbox.try_send(7),
      Err(QueueError::Disconnected) | Err(QueueError::Closed(_))
    ));

    let mut closed_recv = mailbox.recv();
    assert_eq!(poll_once(&mut closed_recv), Poll::Ready(Err(QueueError::Disconnected)));
    assert!(mailbox.is_closed());
  }

  fn poll_once<F>(future: &mut F) -> Poll<F::Output>
  where
    F: Future + ?Sized, {
    let waker = noop_waker();
    let mut context = Context::from_waker(&waker);
    unsafe { Pin::new_unchecked(future) }.poll(&mut context)
  }

  fn noop_waker() -> Waker {
    unsafe { Waker::from_raw(noop_raw_waker()) }
  }

  fn noop_raw_waker() -> RawWaker {
    fn clone(_: *const ()) -> RawWaker {
      noop_raw_waker()
    }
    fn wake(_: *const ()) {}
    fn wake_by_ref(_: *const ()) {}
    fn drop(_: *const ()) {}

    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);

    RawWaker::new(ptr::null(), &VTABLE)
  }
}
