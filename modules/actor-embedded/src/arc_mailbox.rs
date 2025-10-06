use alloc::sync::Arc;

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, RawMutex};
use embassy_sync::signal::{Signal, SignalFuture};

use nexus_actor_core_rs::{Mailbox, MailboxSignal, QueueMailbox, QueueMailboxProducer, QueueMailboxRecv};
use nexus_utils_embedded_rs::collections::queue::mpsc::ArcMpscUnboundedQueue;
use nexus_utils_embedded_rs::{Element, QueueError, QueueSize};

use crate::sync::ArcShared;

#[derive(Clone, Debug)]
pub struct ArcMailbox<M, RM = CriticalSectionRawMutex>
where
  M: Element,
  RM: RawMutex, {
  inner: QueueMailbox<ArcMpscUnboundedQueue<M, RM>, ArcSignal<RM>>,
}

#[derive(Clone, Debug)]
pub struct ArcMailboxSender<M, RM = CriticalSectionRawMutex>
where
  M: Element,
  RM: RawMutex, {
  inner: QueueMailboxProducer<ArcMpscUnboundedQueue<M, RM>, ArcSignal<RM>>,
}

#[derive(Clone, Debug)]
struct ArcSignal<RM>
where
  RM: RawMutex, {
  signal: ArcShared<Signal<RM, ()>>,
}

impl<RM> ArcSignal<RM>
where
  RM: RawMutex,
{
  fn new() -> Self {
    Self {
      signal: ArcShared::new(Signal::new()),
    }
  }
}

impl<RM> MailboxSignal for ArcSignal<RM>
where
  RM: RawMutex,
{
  type WaitFuture<'a>
    = SignalFuture<'a, RM, ()>
  where
    Self: 'a;

  fn notify(&self) {
    self.signal.signal(());
  }

  fn wait(&self) -> Self::WaitFuture<'_> {
    self.signal.wait()
  }
}

impl<M, RM> ArcMailbox<M, RM>
where
  M: Element,
  RM: RawMutex,
{
  pub fn new() -> (Self, ArcMailboxSender<M, RM>) {
    let queue = ArcMpscUnboundedQueue::new();
    let signal = ArcSignal::new();
    Self::with_parts(queue, signal)
  }

  fn with_parts(queue: ArcMpscUnboundedQueue<M, RM>, signal: ArcSignal<RM>) -> (Self, ArcMailboxSender<M, RM>) {
    let mailbox = QueueMailbox::new(queue, signal);
    let sender = ArcMailboxSender {
      inner: mailbox.producer(),
    };
    (Self { inner: mailbox }, sender)
  }

  pub fn producer(&self) -> ArcMailboxSender<M, RM> {
    ArcMailboxSender {
      inner: self.inner.producer(),
    }
  }
}

impl<M, RM> ArcMailboxSender<M, RM>
where
  M: Element,
  RM: RawMutex,
{
  pub fn try_send(&self, message: M) -> Result<(), QueueError<M>> {
    self.inner.try_send(message)
  }

  pub async fn send(&self, message: M) -> Result<(), QueueError<M>> {
    self.try_send(message)
  }
}

pub struct ArcMailboxRecvFuture<'a, M, RM>
where
  M: Element,
  RM: RawMutex, {
  inner: QueueMailboxRecv<'a, ArcMpscUnboundedQueue<M, RM>, ArcSignal<RM>, M>,
}

impl<'a, M, RM> Future for ArcMailboxRecvFuture<'a, M, RM>
where
  M: Element,
  RM: RawMutex,
{
  type Output = M;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    unsafe { self.map_unchecked_mut(|this| &mut this.inner) }.poll(cx)
  }
}

impl<M, RM> Mailbox<M> for ArcMailbox<M, RM>
where
  M: Element,
  RM: RawMutex,
{
  type RecvFuture<'a>
    = ArcMailboxRecvFuture<'a, M, RM>
  where
    Self: 'a;
  type SendError = QueueError<M>;

  fn try_send(&self, message: M) -> Result<(), Self::SendError> {
    self.inner.try_send(message)
  }

  fn recv(&self) -> Self::RecvFuture<'_> {
    ArcMailboxRecvFuture {
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
