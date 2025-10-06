#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

use core::time::Duration;

mod mailbox;
mod spawn;
mod timer;

pub use mailbox::Mailbox;
pub use nexus_utils_core_rs::sync::{Shared, StateCell};
pub use spawn::Spawn;
pub use timer::Timer;

/// Minimal actor loop that waits for messages, handles them, and yields control.
///
/// This is a reference implementation shared by both std and embedded runtimes.
pub async fn actor_loop<M, MB, T, F>(mailbox: &MB, timer: &T, mut handler: F)
where
  MB: Mailbox<M>,
  T: Timer,
  F: FnMut(M), {
  loop {
    let message = mailbox.recv().await;
    handler(message);
    timer.sleep(Duration::from_millis(0)).await;
  }
}

#[cfg(test)]
mod tests {
  extern crate alloc;

  use super::*;
  use alloc::{collections::VecDeque, rc::Rc};
  use core::cell::{Ref, RefCell, RefMut};
  use core::future::Future;
  use core::pin::Pin;
  use core::ptr;
  use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

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

  struct TestMailbox<M> {
    queue: RefCell<VecDeque<M>>,
  }

  impl<M> TestMailbox<M> {
    fn new(messages: impl IntoIterator<Item = M>) -> Self {
      Self {
        queue: RefCell::new(messages.into_iter().collect()),
      }
    }
  }

  impl<M> Mailbox<M> for TestMailbox<M> {
    type RecvFuture<'a>
      = TestMailboxRecv<'a, M>
    where
      Self: 'a;
    type SendError = ();

    fn try_send(&self, message: M) -> Result<(), Self::SendError> {
      self.queue.borrow_mut().push_back(message);
      Ok(())
    }

    fn recv(&self) -> Self::RecvFuture<'_> {
      TestMailboxRecv { mailbox: self }
    }
  }

  struct TestMailboxRecv<'a, M> {
    mailbox: &'a TestMailbox<M>,
  }

  impl<'a, M> Future for TestMailboxRecv<'a, M> {
    type Output = M;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
      if let Some(message) = self.mailbox.queue.borrow_mut().pop_front() {
        Poll::Ready(message)
      } else {
        Poll::Pending
      }
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
    let mailbox = TestMailbox::new([3_u32]);
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
