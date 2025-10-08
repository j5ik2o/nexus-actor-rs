#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::result_large_err)]

#[cfg(feature = "alloc")]
extern crate alloc;

use core::time::Duration;
use nexus_utils_core_rs::QueueError;

mod actor_id;
mod actor_path;
mod context;
mod escalation;
mod failure;
#[cfg(feature = "std")]
mod failure_event_stream;
mod guardian;
mod mailbox;
mod scheduler;
mod spawn;
mod supervisor;
mod system;
mod timer;
mod typed;

pub use actor_id::ActorId;
pub use actor_path::ActorPath;
pub use context::{ActorContext, PriorityActorRef};
pub use escalation::{
  CompositeEscalationSink, CustomEscalationSink, EscalationSink, FailureEventHandler, FailureEventListener,
  ParentGuardianSink, RootEscalationSink,
};
pub use failure::{EscalationStage, FailureEvent, FailureInfo, FailureMetadata};
#[cfg(feature = "std")]
pub use failure_event_stream::{FailureEventHub, FailureEventSubscription};
pub use guardian::{AlwaysRestart, Guardian, GuardianStrategy};
pub use mailbox::SystemMessage;
pub use mailbox::{
  Mailbox, MailboxOptions, MailboxPair, MailboxRuntime, MailboxSignal, PriorityEnvelope, QueueMailbox,
  QueueMailboxProducer, QueueMailboxRecv,
};
pub use nexus_utils_core_rs::{Shared, StateCell};
pub use scheduler::PriorityScheduler;
pub use spawn::Spawn;
pub use supervisor::{NoopSupervisor, Supervisor, SupervisorDirective};
pub use system::{ActorSystem, Props, RootContext};
pub use timer::Timer;
pub use typed::{MessageEnvelope, TypedActorRef, TypedActorSystem, TypedProps, TypedRootContext};

/// Minimal actor loop that waits for messages, handles them, and yields control.
///
/// This is a reference implementation shared by both std and embedded runtimes.
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
