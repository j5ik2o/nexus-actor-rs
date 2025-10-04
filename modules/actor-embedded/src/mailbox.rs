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
