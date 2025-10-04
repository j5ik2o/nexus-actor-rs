#![no_std]

extern crate alloc;

use alloc::collections::VecDeque;
use alloc::rc::Rc;
use core::cell::RefCell;
use core::future::Future;
use core::ops::Deref;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};
use core::time::Duration;

use nexus_actor_core_rs::{Mailbox, Shared, Spawn, Timer};

pub struct RcShared<T>(Rc<T>);

impl<T> RcShared<T> {
  pub fn new(value: T) -> Self {
    Self(Rc::new(value))
  }

  pub fn from_rc(rc: Rc<T>) -> Self {
    Self(rc)
  }

  pub fn into_inner(self) -> Rc<T> {
    self.0
  }
}

impl<T> Clone for RcShared<T> {
  fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}

impl<T> Deref for RcShared<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl<T> Shared<T> for RcShared<T> {
  fn try_unwrap(self) -> Result<T, Self>
  where
    T: Sized, {
    Rc::try_unwrap(self.0).map_err(RcShared)
  }
}

pub struct ImmediateSpawner;

impl Spawn for ImmediateSpawner {
  fn spawn(&self, _fut: impl Future<Output = ()> + 'static) {}
}

pub struct ImmediateTimer;

impl Timer for ImmediateTimer {
  type SleepFuture<'a>
    = core::future::Ready<()>
  where
    Self: 'a;

  fn sleep(&self, _duration: Duration) -> Self::SleepFuture<'_> {
    core::future::ready(())
  }
}

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

pub mod prelude {
  pub use super::{ImmediateSpawner, ImmediateTimer, LocalMailbox, RcShared};
}
