#![no_std]

extern crate alloc;

use alloc::collections::VecDeque;
#[cfg(feature = "embedded_rc")]
use alloc::rc::Rc;
#[cfg(feature = "embedded_arc")]
use alloc::sync::Arc;
use core::cell::RefCell;
#[cfg(feature = "embedded_rc")]
use core::cell::{Ref, RefMut};
use core::future::Future;
#[cfg(feature = "embedded_rc")]
use core::ops::Deref;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};
use core::time::Duration;

#[cfg(feature = "embedded_rc")]
use nexus_actor_core_rs::Shared;
use nexus_actor_core_rs::{Mailbox, Spawn, StateCell, Timer};

#[cfg(feature = "embedded_arc")]
use spin::{Mutex, MutexGuard};

#[cfg(feature = "embedded_rc")]
pub struct RcShared<T>(Rc<T>);

#[cfg(feature = "embedded_rc")]
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

#[cfg(feature = "embedded_rc")]
impl<T> Clone for RcShared<T> {
  fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}

#[cfg(feature = "embedded_rc")]
impl<T> Deref for RcShared<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

#[cfg(feature = "embedded_rc")]
impl<T> Shared<T> for RcShared<T> {
  fn try_unwrap(self) -> Result<T, Self>
  where
    T: Sized, {
    Rc::try_unwrap(self.0).map_err(RcShared)
  }
}

#[cfg(feature = "embedded_rc")]
pub struct RcStateCell<T>(Rc<RefCell<T>>);

#[cfg(feature = "embedded_rc")]
impl<T> RcStateCell<T> {
  pub fn new(value: T) -> Self {
    <Self as StateCell<T>>::new(value)
  }

  pub fn from_rc(rc: Rc<RefCell<T>>) -> Self {
    Self(rc)
  }

  pub fn into_rc(self) -> Rc<RefCell<T>> {
    self.0
  }
}

#[cfg(feature = "embedded_rc")]
impl<T> Clone for RcStateCell<T> {
  fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}

#[cfg(feature = "embedded_rc")]
impl<T> StateCell<T> for RcStateCell<T> {
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
    Self(Rc::new(RefCell::new(value)))
  }

  fn borrow(&self) -> Self::Ref<'_> {
    self.0.borrow()
  }

  fn borrow_mut(&self) -> Self::RefMut<'_> {
    self.0.borrow_mut()
  }
}

#[cfg(feature = "embedded_arc")]
pub struct ArcStateCell<T> {
  inner: Arc<Mutex<T>>,
}

#[cfg(feature = "embedded_arc")]
impl<T> ArcStateCell<T> {
  pub fn new(value: T) -> Self {
    Self {
      inner: Arc::new(Mutex::new(value)),
    }
  }

  pub fn from_arc(inner: Arc<Mutex<T>>) -> Self {
    Self { inner }
  }

  pub fn into_arc(self) -> Arc<Mutex<T>> {
    self.inner
  }
}

#[cfg(feature = "embedded_arc")]
impl<T> Clone for ArcStateCell<T> {
  fn clone(&self) -> Self {
    Self {
      inner: self.inner.clone(),
    }
  }
}

#[cfg(feature = "embedded_arc")]
impl<T> StateCell<T> for ArcStateCell<T> {
  type Ref<'a>
    = MutexGuard<'a, T>
  where
    Self: 'a,
    T: 'a;
  type RefMut<'a>
    = MutexGuard<'a, T>
  where
    Self: 'a,
    T: 'a;

  fn new(value: T) -> Self
  where
    Self: Sized, {
    ArcStateCell::new(value)
  }

  fn borrow(&self) -> Self::Ref<'_> {
    self.inner.lock()
  }

  fn borrow_mut(&self) -> Self::RefMut<'_> {
    self.inner.lock()
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
  #[cfg(feature = "embedded_arc")]
  pub use super::ArcStateCell;
  pub use super::{ImmediateSpawner, ImmediateTimer, LocalMailbox};
  #[cfg(feature = "embedded_rc")]
  pub use super::{RcShared, RcStateCell};
}

#[cfg(test)]
mod tests {
  extern crate std;

  use super::*;

  #[cfg(feature = "embedded_rc")]
  #[test]
  fn rc_state_cell_borrow_mut_applies_changes() {
    let cell = RcStateCell::new(0_u32);

    {
      let mut value = cell.borrow_mut();
      *value = 42;
    }

    assert_eq!(*cell.borrow(), 42);
  }

  #[cfg(feature = "embedded_rc")]
  #[test]
  fn rc_state_cell_clone_shares_state() {
    let cell = RcStateCell::new(10_u32);
    let cloned = cell.clone();

    {
      let mut value = cloned.borrow_mut();
      *value += 5;
    }

    assert_eq!(*cell.borrow(), 15);
  }

  #[cfg(feature = "embedded_arc")]
  #[test]
  fn arc_state_cell_borrow_updates_shared_value() {
    let cell = ArcStateCell::new(1_u32);
    let cloned = cell.clone();

    {
      let mut value = cloned.borrow_mut();
      *value = 7;
    }

    assert_eq!(*cell.borrow(), 7);
  }
}
