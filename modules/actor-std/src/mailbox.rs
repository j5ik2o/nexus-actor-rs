use futures::{future::BoxFuture, FutureExt};
use nexus_actor_core_rs::Mailbox;
use tokio::sync::{mpsc, Mutex as TokioMutex};

/// Tokio mailbox implemented with an `mpsc` channel.
pub struct TokioMailbox<M> {
  pub(crate) sender: mpsc::Sender<M>,
  pub(crate) receiver: TokioMutex<mpsc::Receiver<M>>,
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
