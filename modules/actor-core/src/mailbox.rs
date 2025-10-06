use core::future::Future;

/// Mailbox abstraction that decouples message queue implementations from core logic.
pub trait Mailbox<M> {
  type SendError;
  type RecvFuture<'a>: Future<Output = M> + 'a
  where
    Self: 'a;

  fn try_send(&self, message: M) -> Result<(), Self::SendError>;
  fn recv(&self) -> Self::RecvFuture<'_>;
}
