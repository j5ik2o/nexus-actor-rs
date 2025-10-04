use core::future::Future;
use core::time::Duration;

use futures::{future::BoxFuture, FutureExt};
use nexus_actor_core_rs::{Mailbox, Spawn, Timer};
use tokio::sync::{mpsc, Mutex};
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
  receiver: Mutex<mpsc::Receiver<M>>,
}

impl<M> TokioMailbox<M> {
  pub fn new(capacity: usize) -> (Self, mpsc::Sender<M>) {
    let (sender, receiver) = mpsc::channel(capacity);
    let mailbox = Self {
      sender: sender.clone(),
      receiver: Mutex::new(receiver),
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

pub mod prelude {
  pub use super::{TokioMailbox, TokioSpawner, TokioTimer};
  pub use nexus_actor_core_rs::actor_loop;
}
