use core::future::Future;
use core::time::Duration;

use futures::{future::BoxFuture, FutureExt};
use nexus_actor_core_rs::{Mailbox, Spawn, StateCell, Timer};
use std::sync::{Arc, Mutex, MutexGuard};
use tokio::sync::{mpsc, Mutex as TokioMutex};
use tokio::time::Sleep;

/// Shared spawn adapter built on top of `tokio::spawn`.
pub struct TokioSpawner;

impl Spawn for TokioSpawner {
  fn spawn(&self, fut: impl Future<Output = ()> + Send + 'static) {
    tokio::spawn(fut);
  }
}

/// Tokio-backed timer implementation.
pub struct TokioTimer;

impl Timer for TokioTimer {
  type SleepFuture<'a>
    = Sleep
  where
    Self: 'a;

  fn sleep(&self, duration: Duration) -> Self::SleepFuture<'_> {
    tokio::time::sleep(duration)
  }
}

/// Tokio mailbox implemented with an `mpsc` channel.
pub struct TokioMailbox<M> {
  sender: mpsc::Sender<M>,
  receiver: TokioMutex<mpsc::Receiver<M>>,
}

impl<M> TokioMailbox<M> {
  pub fn new(capacity: usize) -> (Self, mpsc::Sender<M>) {
    let (sender, receiver) = mpsc::channel(capacity);
    let mailbox = Self {
      sender: sender.clone(),
      receiver: TokioMutex::new(receiver),
    };
    (mailbox, sender)
  }
}

impl<M: Send + 'static> Mailbox<M> for TokioMailbox<M> {
  type RecvFuture<'a>
    = BoxFuture<'a, M>
  where
    Self: 'a;
  type SendError = mpsc::error::TrySendError<M>;

  fn try_send(&self, message: M) -> Result<(), Self::SendError> {
    self.sender.try_send(message)
  }

  fn recv(&self) -> Self::RecvFuture<'_> {
    async move {
      let mut guard = self.receiver.lock().await;
      guard.recv().await.expect("mailbox closed")
    }
    .boxed()
  }
}

/// Arc-based state cell backed by `std::sync::Mutex` for synchronous access on std runtimes.
pub struct ArcStateCell<T>(Arc<Mutex<T>>);

impl<T> ArcStateCell<T> {
  pub fn new(value: T) -> Self {
    Self(Arc::new(Mutex::new(value)))
  }

  pub fn from_arc(inner: Arc<Mutex<T>>) -> Self {
    Self(inner)
  }

  pub fn into_arc(self) -> Arc<Mutex<T>> {
    self.0
  }
}

impl<T> Clone for ArcStateCell<T> {
  fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}

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
    self.0.lock().expect("mutex poisoned")
  }

  fn borrow_mut(&self) -> Self::RefMut<'_> {
    self.0.lock().expect("mutex poisoned")
  }
}

pub mod prelude {
  pub use super::{ArcStateCell, TokioMailbox, TokioSpawner, TokioTimer};
  pub use nexus_actor_core_rs::actor_loop;
}

#[cfg(test)]
mod tests {
  use super::*;
  use nexus_actor_core_rs::actor_loop;

  #[tokio::test(flavor = "current_thread")]
  async fn test_actor_loop_updates_state() {
    let (mailbox, sender) = TokioMailbox::new(8);
    let mailbox = Arc::new(mailbox);
    let state = ArcStateCell::new(0_u32);

    let actor_state = state.clone();
    let actor_mailbox = mailbox.clone();

    TokioSpawner.spawn(async move {
      let timer = TokioTimer;
      actor_loop(actor_mailbox.as_ref(), &timer, move |msg: u32| {
        let mut guard = actor_state.borrow_mut();
        *guard += msg;
      })
      .await;
    });

    sender.send(4_u32).await.expect("send message");
    tokio::time::sleep(Duration::from_millis(10)).await;

    assert_eq!(*state.borrow(), 4_u32);
  }
}
