use alloc::rc::Rc;
use core::cell::RefCell;
use core::fmt;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};

use nexus_actor_core_rs::{Mailbox, MailboxSignal, QueueMailbox, QueueMailboxProducer, QueueMailboxRecv};
use nexus_utils_embedded_rs::collections::queue::mpsc::RcMpscUnboundedQueue;
use nexus_utils_embedded_rs::{Element, QueueError, QueueSize};

pub struct LocalMailbox<M>
where
  M: Element, {
  inner: QueueMailbox<RcMpscUnboundedQueue<M>, LocalSignal>,
}

pub struct LocalMailboxSender<M>
where
  M: Element, {
  inner: QueueMailboxProducer<RcMpscUnboundedQueue<M>, LocalSignal>,
}

#[derive(Clone, Debug, Default)]
struct LocalSignal {
  state: Rc<RefCell<SignalState>>,
}

#[derive(Debug, Default)]
struct SignalState {
  notified: bool,
  waker: Option<Waker>,
}

impl MailboxSignal for LocalSignal {
  type WaitFuture<'a>
    = LocalSignalWait
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
    LocalSignalWait { signal: self.clone() }
  }
}

struct LocalSignalWait {
  signal: LocalSignal,
}

impl Future for LocalSignalWait {
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

impl<M> LocalMailbox<M>
where
  M: Element,
  RcMpscUnboundedQueue<M>: Clone,
{
  pub fn new() -> (Self, LocalMailboxSender<M>) {
    let queue = RcMpscUnboundedQueue::new();
    let signal = LocalSignal::default();
    Self::with_parts(queue, signal)
  }

  pub fn producer(&self) -> LocalMailboxSender<M> {
    LocalMailboxSender {
      inner: self.inner.producer(),
    }
  }

  fn with_parts(queue: RcMpscUnboundedQueue<M>, signal: LocalSignal) -> (Self, LocalMailboxSender<M>) {
    let mailbox = QueueMailbox::new(queue, signal);
    let sender = LocalMailboxSender {
      inner: mailbox.producer(),
    };
    (Self { inner: mailbox }, sender)
  }
}

impl<M> LocalMailboxSender<M>
where
  M: Element,
  RcMpscUnboundedQueue<M>: Clone,
{
  pub fn try_send(&self, message: M) -> Result<(), QueueError<M>> {
    self.inner.try_send(message)
  }

  pub async fn send(&self, message: M) -> Result<(), QueueError<M>> {
    self.try_send(message)
  }
}

pub struct LocalMailboxRecvFuture<'a, M>
where
  M: Element,
  RcMpscUnboundedQueue<M>: Clone, {
  inner: QueueMailboxRecv<'a, RcMpscUnboundedQueue<M>, LocalSignal, M>,
}

impl<'a, M> Future for LocalMailboxRecvFuture<'a, M>
where
  M: Element,
  RcMpscUnboundedQueue<M>: Clone,
{
  type Output = M;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    unsafe { self.map_unchecked_mut(|this| &mut this.inner) }.poll(cx)
  }
}

impl<M> Clone for LocalMailbox<M>
where
  M: Element,
  RcMpscUnboundedQueue<M>: Clone,
{
  fn clone(&self) -> Self {
    Self {
      inner: self.inner.clone(),
    }
  }
}

impl<M> fmt::Debug for LocalMailbox<M>
where
  M: Element,
{
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("LocalMailbox").finish()
  }
}

impl<M> Clone for LocalMailboxSender<M>
where
  M: Element,
  RcMpscUnboundedQueue<M>: Clone,
{
  fn clone(&self) -> Self {
    Self {
      inner: self.inner.clone(),
    }
  }
}

impl<M> fmt::Debug for LocalMailboxSender<M>
where
  M: Element,
{
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("LocalMailboxSender").finish()
  }
}

impl<M> Mailbox<M> for LocalMailbox<M>
where
  M: Element,
  RcMpscUnboundedQueue<M>: Clone,
{
  type RecvFuture<'a>
    = LocalMailboxRecvFuture<'a, M>
  where
    Self: 'a;
  type SendError = QueueError<M>;

  fn try_send(&self, message: M) -> Result<(), Self::SendError> {
    self.inner.try_send(message)
  }

  fn recv(&self) -> Self::RecvFuture<'_> {
    LocalMailboxRecvFuture {
      inner: self.inner.recv(),
    }
  }

  fn len(&self) -> QueueSize {
    self.inner.len()
  }

  fn capacity(&self) -> QueueSize {
    self.inner.capacity()
  }

  fn close(&self) {
    self.inner.close();
  }

  fn is_closed(&self) -> bool {
    self.inner.is_closed()
  }
}

#[cfg(test)]
mod tests {
  extern crate std;

  use super::*;
  use core::task::{Context, Poll};
  use std::future::Future;
  use std::pin::Pin;
  use std::sync::Arc;
  use std::task::Wake;

  fn noop_waker() -> core::task::Waker {
    struct NoopWake;
    impl Wake for NoopWake {
      fn wake(self: Arc<Self>) {}

      fn wake_by_ref(self: &Arc<Self>) {}
    }
    core::task::Waker::from(Arc::new(NoopWake))
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
    let (mailbox, sender) = LocalMailbox::<u32>::new();
    sender.try_send(1).unwrap();
    sender.try_send(2).unwrap();

    let future = mailbox.recv();
    let (first_poll, future) = pin_poll(future);
    assert_eq!(first_poll, Poll::Ready(1));

    let (second_poll, _) = pin_poll(future);
    assert_eq!(second_poll, Poll::Ready(2));
  }

  #[test]
  fn local_mailbox_wakes_after_message_arrives() {
    let (mailbox, sender) = LocalMailbox::new();

    let mut future = mailbox.recv();
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    let mut pinned = unsafe { Pin::new_unchecked(&mut future) };

    assert!(pinned.as_mut().poll(&mut cx).is_pending());

    sender.try_send(99_u8).unwrap();

    assert_eq!(pinned.poll(&mut cx), Poll::Ready(99));
  }

  #[test]
  fn local_mailbox_preserves_messages_post_wake() {
    let (mailbox, sender) = LocalMailbox::new();

    let mut recv_future = mailbox.recv();
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    let mut pinned = unsafe { Pin::new_unchecked(&mut recv_future) };

    assert!(pinned.as_mut().poll(&mut cx).is_pending());
    sender.try_send(7_u8).unwrap();

    let value = pinned.poll(&mut cx);
    assert_eq!(value, Poll::Ready(7));
  }
}
