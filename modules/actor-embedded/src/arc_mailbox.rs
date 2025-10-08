use alloc::boxed::Box;

use core::future::Future;
use core::marker::PhantomData;
use core::pin::Pin;
use core::task::{Context, Poll};

use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, RawMutex};
use embassy_sync::signal::Signal;

use nexus_actor_core_rs::{
  Mailbox, MailboxOptions, MailboxPair, MailboxRuntime, MailboxSignal, QueueMailbox, QueueMailboxProducer,
  QueueMailboxRecv,
};
use nexus_utils_embedded_rs::collections::queue::mpsc::ArcMpscUnboundedQueue;
use nexus_utils_embedded_rs::{Element, QueueError, QueueSize};

use nexus_utils_embedded_rs::sync::ArcShared;

#[derive(Clone)]
pub struct ArcMailbox<M, RM = CriticalSectionRawMutex>
where
  M: Element,
  RM: RawMutex,
{
  inner: QueueMailbox<ArcMpscUnboundedQueue<M, RM>, ArcSignal<RM>>,
}

#[derive(Clone)]
pub struct ArcMailboxSender<M, RM = CriticalSectionRawMutex>
where
  M: Element,
  RM: RawMutex,
{
  inner: QueueMailboxProducer<ArcMpscUnboundedQueue<M, RM>, ArcSignal<RM>>,
}

#[derive(Clone)]
pub struct ArcMailboxRuntime<RM = CriticalSectionRawMutex>
where
  RM: RawMutex,
{
  _marker: PhantomData<RM>,
}

impl<RM> Default for ArcMailboxRuntime<RM>
where
  RM: RawMutex,
{
  fn default() -> Self {
    Self::new()
  }
}

pub struct ArcSignal<RM>
where
  RM: RawMutex,
{
  signal: ArcShared<Signal<RM, ()>>,
}

impl<RM> Clone for ArcSignal<RM>
where
  RM: RawMutex,
{
  fn clone(&self) -> Self {
    Self {
      signal: self.signal.clone(),
    }
  }
}

impl<RM> Default for ArcSignal<RM>
where
  RM: RawMutex,
{
  fn default() -> Self {
    Self {
      signal: ArcShared::new(Signal::new()),
    }
  }
}

impl<RM> ArcSignal<RM>
where
  RM: RawMutex,
{
  fn new() -> Self {
    Self::default()
  }
}

impl<RM> MailboxSignal for ArcSignal<RM>
where
  RM: RawMutex,
{
  type WaitFuture<'a>
    = ArcSignalWait<'a, RM>
  where
    Self: 'a;

  fn notify(&self) {
    self.signal.signal(());
  }

  fn wait(&self) -> Self::WaitFuture<'_> {
    ArcSignalWait {
      future: Box::pin(self.signal.wait()),
      _marker: PhantomData,
    }
  }
}

pub struct ArcSignalWait<'a, RM>
where
  RM: RawMutex,
{
  future: Pin<Box<dyn Future<Output = ()> + 'a>>,
  _marker: PhantomData<RM>,
}

impl<'a, RM> Future for ArcSignalWait<'a, RM>
where
  RM: RawMutex,
{
  type Output = ();

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let this = unsafe { self.get_unchecked_mut() };
    this.future.as_mut().poll(cx)
  }
}

impl<RM> ArcMailboxRuntime<RM>
where
  RM: RawMutex,
{
  pub const fn new() -> Self {
    Self { _marker: PhantomData }
  }

  pub fn mailbox<M>(&self, options: MailboxOptions) -> (ArcMailbox<M, RM>, ArcMailboxSender<M, RM>)
  where
    M: Element,
  {
    let (mailbox, sender) = self.build_mailbox::<M>(options);
    (ArcMailbox { inner: mailbox }, ArcMailboxSender { inner: sender })
  }

  pub fn unbounded<M>(&self) -> (ArcMailbox<M, RM>, ArcMailboxSender<M, RM>)
  where
    M: Element,
  {
    self.mailbox(MailboxOptions::unbounded())
  }
}

impl<RM> MailboxRuntime for ArcMailboxRuntime<RM>
where
  RM: RawMutex,
{
  type Queue<M>
    = ArcMpscUnboundedQueue<M, RM>
  where
    M: Element;
  type Signal = ArcSignal<RM>;

  fn build_mailbox<M>(&self, _options: MailboxOptions) -> MailboxPair<Self::Queue<M>, Self::Signal>
  where
    M: Element,
  {
    let queue = ArcMpscUnboundedQueue::new();
    let signal = ArcSignal::new();
    let mailbox = QueueMailbox::new(queue, signal);
    let sender = mailbox.producer();
    (mailbox, sender)
  }
}

impl<M, RM> ArcMailbox<M, RM>
where
  M: Element,
  RM: RawMutex,
{
  pub fn new() -> (Self, ArcMailboxSender<M, RM>) {
    ArcMailboxRuntime::<RM>::new().unbounded()
  }

  pub fn inner(&self) -> &QueueMailbox<ArcMpscUnboundedQueue<M, RM>, ArcSignal<RM>> {
    &self.inner
  }
}

impl<M, RM> Mailbox<M> for ArcMailbox<M, RM>
where
  M: Element,
  RM: RawMutex,
  ArcMpscUnboundedQueue<M, RM>: Clone,
{
  type RecvFuture<'a>
    = QueueMailboxRecv<'a, ArcMpscUnboundedQueue<M, RM>, ArcSignal<RM>, M>
  where
    Self: 'a;
  type SendError = QueueError<M>;

  fn try_send(&self, message: M) -> Result<(), Self::SendError> {
    self.inner.try_send(message)
  }

  fn recv(&self) -> Self::RecvFuture<'_> {
    self.inner.recv()
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

impl<M, RM> ArcMailboxSender<M, RM>
where
  M: Element,
  RM: RawMutex,
  ArcMpscUnboundedQueue<M, RM>: Clone,
{
  pub fn try_send(&self, message: M) -> Result<(), QueueError<M>> {
    self.inner.try_send(message)
  }

  pub async fn send(&self, message: M) -> Result<(), QueueError<M>> {
    self.inner.send(message).await
  }

  pub fn inner(&self) -> &QueueMailboxProducer<ArcMpscUnboundedQueue<M, RM>, ArcSignal<RM>> {
    &self.inner
  }
}
