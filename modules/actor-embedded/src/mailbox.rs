use alloc::collections::VecDeque;
use core::cell::RefCell;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};

use nexus_actor_core_rs::Mailbox;

pub struct LocalMailbox<M> {
  queue: RefCell<VecDeque<M>>,
  waker: RefCell<Option<Waker>>,
}

impl<M> LocalMailbox<M> {
  pub const fn new() -> Self {
    Self {
      queue: RefCell::new(VecDeque::new()),
      waker: RefCell::new(None),
    }
  }
}

impl<M> Mailbox<M> for LocalMailbox<M> {
  type RecvFuture<'a>
    = LocalMailboxRecv<'a, M>
  where
    Self: 'a;
  type SendError = ();

  fn try_send(&self, message: M) -> Result<(), Self::SendError> {
    self.queue.borrow_mut().push_back(message);
    if let Some(waker) = self.waker.borrow_mut().take() {
      waker.wake();
    }
    Ok(())
  }

  fn recv(&self) -> Self::RecvFuture<'_> {
    LocalMailboxRecv { mailbox: self }
  }
}

pub struct LocalMailboxRecv<'a, M> {
  mailbox: &'a LocalMailbox<M>,
}

impl<'a, M> Future for LocalMailboxRecv<'a, M> {
  type Output = M;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    if let Some(message) = self.mailbox.queue.borrow_mut().pop_front() {
      Poll::Ready(message)
    } else {
      self.mailbox.waker.replace(Some(cx.waker().clone()));
      Poll::Pending
    }
  }
}

#[cfg(test)]
mod tests {
  extern crate std;

  use super::*;
  use std::cell::Cell;
  use std::future::Future;
  use std::pin::Pin;
  use std::sync::Arc;
  use std::task::{Context, Poll, Wake, Waker};

  fn noop_waker() -> Waker {
    struct NoopWake;
    impl Wake for NoopWake {
      fn wake(self: Arc<Self>) {}

      fn wake_by_ref(self: &Arc<Self>) {}
    }
    Waker::from(Arc::new(NoopWake))
  }

  fn pin_poll<F: Future>(mut fut: F) -> (Poll<F::Output>, F)
  where
    F: Unpin, {
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    let poll = Pin::new(&mut fut).poll(&mut cx);
    (poll, fut)
  }

  #[test]
  fn local_mailbox_delivers_messages_in_fifo_order() {
    let mailbox = LocalMailbox::new();
    mailbox.try_send(1_u32).unwrap();
    mailbox.try_send(2_u32).unwrap();

    let future = mailbox.recv();
    let (first_poll, future) = pin_poll(future);
    assert_eq!(first_poll, Poll::Ready(1));

    let (second_poll, _) = pin_poll(future);
    assert_eq!(second_poll, Poll::Ready(2));
  }

  #[test]
  fn local_mailbox_wakes_after_message_arrives() {
    let mailbox = LocalMailbox::new();

    let mut future = mailbox.recv();
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    let mut pinned = unsafe { Pin::new_unchecked(&mut future) };

    assert!(pinned.as_mut().poll(&mut cx).is_pending());

    mailbox.try_send(99_u8).unwrap();

    assert_eq!(pinned.poll(&mut cx), Poll::Ready(99));
  }

  #[test]
  fn local_mailbox_preserves_messages_post_wake() {
    let mailbox = LocalMailbox::new();
    let flag = Cell::new(0_u8);

    let mut recv_future = mailbox.recv();
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    let mut pinned = unsafe { Pin::new_unchecked(&mut recv_future) };

    assert!(pinned.as_mut().poll(&mut cx).is_pending());
    mailbox.try_send(7_u8).unwrap();

    if let Poll::Ready(val) = pinned.as_mut().poll(&mut cx) {
      flag.set(val);
    }

    assert_eq!(flag.get(), 7);
  }
}
