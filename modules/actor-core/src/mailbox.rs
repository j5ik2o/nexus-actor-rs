use core::future::Future;
use core::marker::PhantomData;
use core::pin::Pin;
use core::task::{Context, Poll};

use nexus_utils_core_rs::sync::Flag;
use nexus_utils_core_rs::{Element, PriorityMessage, QueueError, QueueRw, QueueSize, DEFAULT_PRIORITY};

use crate::actor_id::ActorId;

/// Mailbox abstraction that decouples message queue implementations from core logic.
pub trait Mailbox<M> {
  type SendError;
  type RecvFuture<'a>: Future<Output = M> + 'a
  where
    Self: 'a;

  fn try_send(&self, message: M) -> Result<(), Self::SendError>;
  fn recv(&self) -> Self::RecvFuture<'_>;

  fn len(&self) -> QueueSize {
    QueueSize::limitless()
  }

  fn capacity(&self) -> QueueSize {
    QueueSize::limitless()
  }

  fn close(&self) {}

  fn is_closed(&self) -> bool {
    false
  }
}

/// Notification primitive used by [`QueueMailbox`] to park awaiting receivers until
/// new messages are available.
pub trait MailboxSignal: Clone {
  type WaitFuture<'a>: Future<Output = ()> + 'a
  where
    Self: 'a;

  fn notify(&self);
  fn wait(&self) -> Self::WaitFuture<'_>;
}

/// 制御メッセージかどうかを区別するチャネル種別。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PriorityChannel {
  Regular,
  Control,
}

/// protoactor-go の `SystemMessage` を参考にした制御メッセージ種別。
/// フィールド構成は現段階の要件に絞り、必要に応じて拡張する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SystemMessage {
  Watch(ActorId),
  Unwatch(ActorId),
  Stop,
  Failure,
  Restart,
  Suspend,
  Resume,
}

impl SystemMessage {
  /// 推奨優先度。protoactor-go の優先度テーブルをベースに設定。
  pub fn priority(&self) -> i8 {
    match self {
      SystemMessage::Watch(_) | SystemMessage::Unwatch(_) => DEFAULT_PRIORITY + 5,
      SystemMessage::Stop => DEFAULT_PRIORITY + 10,
      SystemMessage::Failure => DEFAULT_PRIORITY + 12,
      SystemMessage::Restart => DEFAULT_PRIORITY + 11,
      SystemMessage::Suspend | SystemMessage::Resume => DEFAULT_PRIORITY + 9,
    }
  }
}

impl Element for SystemMessage {}

/// Envelope型: 優先度付きメッセージを格納し、`PriorityMessage` を実装する。
#[allow(dead_code)]
#[derive(Debug)]
pub struct PriorityEnvelope<M> {
  message: M,
  priority: i8,
  channel: PriorityChannel,
}

#[allow(dead_code)]
impl<M> PriorityEnvelope<M> {
  pub fn new(message: M, priority: i8) -> Self {
    Self::with_channel(message, priority, PriorityChannel::Regular)
  }

  pub fn with_channel(message: M, priority: i8, channel: PriorityChannel) -> Self {
    Self {
      message,
      priority,
      channel,
    }
  }

  pub fn control(message: M, priority: i8) -> Self {
    Self::with_channel(message, priority, PriorityChannel::Control)
  }

  pub fn message(&self) -> &M {
    &self.message
  }

  pub fn priority(&self) -> i8 {
    self.priority
  }

  pub fn channel(&self) -> PriorityChannel {
    self.channel
  }

  pub fn is_control(&self) -> bool {
    matches!(self.channel, PriorityChannel::Control)
  }

  pub fn into_parts(self) -> (M, i8) {
    (self.message, self.priority)
  }

  pub fn into_parts_with_channel(self) -> (M, i8, PriorityChannel) {
    (self.message, self.priority, self.channel)
  }

  pub fn map<N>(self, f: impl FnOnce(M) -> N) -> PriorityEnvelope<N> {
    PriorityEnvelope {
      message: f(self.message),
      priority: self.priority,
      channel: self.channel,
    }
  }

  pub fn map_priority(mut self, f: impl FnOnce(i8) -> i8) -> Self {
    self.priority = f(self.priority);
    self
  }
}

#[allow(dead_code)]
impl<M> PriorityEnvelope<M>
where
  M: Element,
{
  pub fn with_default_priority(message: M) -> Self {
    Self::new(message, DEFAULT_PRIORITY)
  }
}

impl PriorityEnvelope<SystemMessage> {
  /// SystemMessage 用の Envelope ヘルパー。
  pub fn from_system(message: SystemMessage) -> Self {
    let priority = message.priority();
    PriorityEnvelope::with_channel(message, priority, PriorityChannel::Control)
  }
}

impl<M> PriorityMessage for PriorityEnvelope<M>
where
  M: Element,
{
  fn get_priority(&self) -> Option<i8> {
    Some(self.priority)
  }
}

impl<M> Element for PriorityEnvelope<M> where M: Element {}

/// Pair of mailbox body and its sending handle returned by runtime builders.
pub type MailboxPair<Q, S> = (QueueMailbox<Q, S>, QueueMailboxProducer<Q, S>);

/// Runtime-agnostic construction options for [`QueueMailbox`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MailboxOptions {
  pub capacity: QueueSize,
  pub priority_capacity: QueueSize,
}

impl MailboxOptions {
  pub const fn with_capacity(capacity: usize) -> Self {
    Self {
      capacity: QueueSize::limited(capacity),
      priority_capacity: QueueSize::limitless(),
    }
  }

  pub const fn with_capacities(capacity: QueueSize, priority_capacity: QueueSize) -> Self {
    Self {
      capacity,
      priority_capacity,
    }
  }

  pub const fn with_priority_capacity(mut self, priority_capacity: QueueSize) -> Self {
    self.priority_capacity = priority_capacity;
    self
  }

  pub const fn unbounded() -> Self {
    Self {
      capacity: QueueSize::limitless(),
      priority_capacity: QueueSize::limitless(),
    }
  }
}

impl Default for MailboxOptions {
  fn default() -> Self {
    Self::unbounded()
  }
}

/// Trait implemented by runtime-specific mailbox builders (Tokio, Embassy, etc.).
pub trait MailboxRuntime {
  type Signal: MailboxSignal;

  type Queue<M>: QueueRw<M> + Clone
  where
    M: Element;

  fn build_mailbox<M>(&self, options: MailboxOptions) -> MailboxPair<Self::Queue<M>, Self::Signal>
  where
    M: Element;

  fn build_default_mailbox<M>(&self) -> MailboxPair<Self::Queue<M>, Self::Signal>
  where
    M: Element, {
    self.build_mailbox(MailboxOptions::default())
  }
}

/// Mailbox implementation backed by a generic queue and notification signal.
#[derive(Debug)]
pub struct QueueMailbox<Q, S> {
  queue: Q,
  signal: S,
  closed: Flag,
}

impl<Q, S> QueueMailbox<Q, S> {
  pub fn new(queue: Q, signal: S) -> Self {
    Self {
      queue,
      signal,
      closed: Flag::default(),
    }
  }

  pub fn queue(&self) -> &Q {
    &self.queue
  }

  pub fn signal(&self) -> &S {
    &self.signal
  }

  pub fn producer(&self) -> QueueMailboxProducer<Q, S>
  where
    Q: Clone,
    S: Clone, {
    QueueMailboxProducer {
      queue: self.queue.clone(),
      signal: self.signal.clone(),
      closed: self.closed.clone(),
    }
  }
}

impl<Q, S> Clone for QueueMailbox<Q, S>
where
  Q: Clone,
  S: Clone,
{
  fn clone(&self) -> Self {
    Self {
      queue: self.queue.clone(),
      signal: self.signal.clone(),
      closed: self.closed.clone(),
    }
  }
}

/// Sending handle that shares queue ownership with [`QueueMailbox`].
#[derive(Clone, Debug)]
pub struct QueueMailboxProducer<Q, S> {
  queue: Q,
  signal: S,
  closed: Flag,
}

impl<Q, S> QueueMailboxProducer<Q, S> {
  pub fn try_send<M>(&self, message: M) -> Result<(), QueueError<M>>
  where
    Q: QueueRw<M>,
    S: MailboxSignal,
    M: Element, {
    if self.closed.get() {
      return Err(QueueError::Disconnected);
    }

    match self.queue.offer(message) {
      Ok(()) => {
        self.signal.notify();
        Ok(())
      }
      Err(err @ QueueError::Disconnected) | Err(err @ QueueError::Closed(_)) => {
        self.closed.set(true);
        Err(err)
      }
      Err(err) => Err(err),
    }
  }

  pub async fn send<M>(&self, message: M) -> Result<(), QueueError<M>>
  where
    Q: QueueRw<M>,
    S: MailboxSignal,
    M: Element, {
    self.try_send(message)
  }
}

impl<M, Q, S> Mailbox<M> for QueueMailbox<Q, S>
where
  Q: QueueRw<M>,
  S: MailboxSignal,
  M: Element,
{
  type RecvFuture<'a>
    = QueueMailboxRecv<'a, Q, S, M>
  where
    Self: 'a;
  type SendError = QueueError<M>;

  fn try_send(&self, message: M) -> Result<(), Self::SendError> {
    match self.queue.offer(message) {
      Ok(()) => {
        self.signal.notify();
        Ok(())
      }
      Err(err @ QueueError::Disconnected) | Err(err @ QueueError::Closed(_)) => {
        self.closed.set(true);
        Err(err)
      }
      Err(err) => Err(err),
    }
  }

  fn recv(&self) -> Self::RecvFuture<'_> {
    QueueMailboxRecv {
      mailbox: self,
      wait: None,
      marker: PhantomData,
    }
  }

  fn len(&self) -> QueueSize {
    self.queue.len()
  }

  fn capacity(&self) -> QueueSize {
    self.queue.capacity()
  }

  fn close(&self) {
    self.queue.clean_up();
    self.signal.notify();
    self.closed.set(true);
  }

  fn is_closed(&self) -> bool {
    self.closed.get()
  }
}

pub struct QueueMailboxRecv<'a, Q, S, M>
where
  Q: QueueRw<M>,
  S: MailboxSignal,
  M: Element, {
  mailbox: &'a QueueMailbox<Q, S>,
  wait: Option<S::WaitFuture<'a>>,
  marker: PhantomData<M>,
}

impl<'a, Q, S, M> Future for QueueMailboxRecv<'a, Q, S, M>
where
  Q: QueueRw<M>,
  S: MailboxSignal,
  M: Element,
{
  type Output = M;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let this = unsafe { self.get_unchecked_mut() };
    if this.mailbox.closed.get() {
      return Poll::Pending;
    }
    loop {
      match this.mailbox.queue.poll() {
        Ok(Some(message)) => {
          this.wait = None;
          return Poll::Ready(message);
        }
        Ok(None) => {
          if this.wait.is_none() {
            this.wait = Some(this.mailbox.signal.wait());
          }
        }
        Err(QueueError::Disconnected) | Err(QueueError::Closed(_)) => {
          this.mailbox.closed.set(true);
          this.wait = None;
          return Poll::Pending;
        }
        Err(QueueError::Full(_)) | Err(QueueError::OfferError(_)) => {
          return Poll::Pending;
        }
      }

      if let Some(wait) = this.wait.as_mut() {
        match unsafe { Pin::new_unchecked(wait) }.poll(cx) {
          Poll::Ready(()) => {
            this.wait = None;
            continue;
          }
          Poll::Pending => return Poll::Pending,
        }
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn mailbox_options_helpers_cover_basic_cases() {
    let limited = MailboxOptions::with_capacity(4);
    assert_eq!(limited.capacity, QueueSize::limited(4));
    assert!(!limited.capacity.is_limitless());

    let unbounded = MailboxOptions::unbounded();
    assert!(unbounded.capacity.is_limitless());

    let defaulted = MailboxOptions::default();
    assert_eq!(defaulted.capacity, QueueSize::limitless());
  }

  #[test]
  fn priority_envelope_exposes_priority() {
    let envelope = PriorityEnvelope::new(42_u8, 5);
    assert_eq!(envelope.priority(), 5);
    assert_eq!(*envelope.message(), 42);
    let raised = PriorityEnvelope::new(42_u8, 5).map_priority(|p| p + 1);
    assert_eq!(raised.priority(), 6);

    let default = PriorityEnvelope::with_default_priority(7_u8);
    assert_eq!(default.priority(), DEFAULT_PRIORITY);

    let (message, priority) = PriorityEnvelope::new(42_u8, 5).into_parts();
    assert_eq!(message, 42);
    assert_eq!(priority, 5);

    let control = PriorityEnvelope::control(99_u8, 9);
    assert!(control.is_control());
    let (msg, pri, channel) = control.into_parts_with_channel();
    assert_eq!(msg, 99);
    assert_eq!(pri, 9);
    assert_eq!(channel, PriorityChannel::Control);

    let sys_env = PriorityEnvelope::<SystemMessage>::from_system(SystemMessage::Stop);
    assert!(sys_env.is_control());
    assert!(sys_env.priority() > DEFAULT_PRIORITY);
  }
}

#[cfg(any(test, feature = "test-support"))]
pub mod test_support {
  use super::*;
  use alloc::rc::Rc;
  use core::cell::RefCell;
  use core::fmt;

  use nexus_utils_core_rs::{MpscBuffer, MpscHandle, MpscQueue, QueueSize, RingBufferBackend, Shared};

  #[derive(Clone, Debug, Default)]
  pub struct TestMailboxRuntime {
    capacity: Option<usize>,
  }

  impl TestMailboxRuntime {
    pub fn new(capacity: Option<usize>) -> Self {
      Self { capacity }
    }

    pub fn with_capacity_per_queue(capacity: usize) -> Self {
      Self::new(Some(capacity))
    }

    pub fn unbounded() -> Self {
      Self::default()
    }

    fn resolve_capacity(&self, options: MailboxOptions) -> Option<usize> {
      match options.capacity {
        QueueSize::Limitless => self.capacity,
        QueueSize::Limited(value) => Some(value),
      }
    }
  }

  pub struct RcBackendHandle<T>(Rc<RingBufferBackend<RefCell<MpscBuffer<T>>>>);

  impl<T> Clone for RcBackendHandle<T> {
    fn clone(&self) -> Self {
      Self(self.0.clone())
    }
  }

  impl<T> RcBackendHandle<T> {
    fn new(capacity: Option<usize>) -> Self {
      let buffer = RefCell::new(MpscBuffer::new(capacity));
      let backend = RingBufferBackend::new(buffer);
      Self(Rc::new(backend))
    }
  }

  impl<T> core::ops::Deref for RcBackendHandle<T> {
    type Target = RingBufferBackend<RefCell<MpscBuffer<T>>>;

    fn deref(&self) -> &Self::Target {
      &self.0
    }
  }

  impl<T> fmt::Debug for RcBackendHandle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      f.debug_struct("RcBackendHandle").finish()
    }
  }

  impl<T> Shared<RingBufferBackend<RefCell<MpscBuffer<T>>>> for RcBackendHandle<T> {}

  impl<T> MpscHandle<T> for RcBackendHandle<T> {
    type Backend = RingBufferBackend<RefCell<MpscBuffer<T>>>;

    fn backend(&self) -> &Self::Backend {
      &self.0
    }
  }

  pub type TestQueue<M> = MpscQueue<RcBackendHandle<M>, M>;

  #[derive(Clone, Default)]
  pub struct TestSignal {
    state: Rc<RefCell<TestSignalState>>,
  }

  #[derive(Clone, Default)]
  struct TestSignalState {
    notified: bool,
    waker: Option<core::task::Waker>,
  }

  impl MailboxSignal for TestSignal {
    type WaitFuture<'a>
      = TestSignalWait<'a>
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
      TestSignalWait {
        signal: self.clone(),
        _marker: PhantomData,
      }
    }
  }

  pub struct TestSignalWait<'a> {
    signal: TestSignal,
    _marker: PhantomData<&'a ()>,
  }

  impl<'a> Future for TestSignalWait<'a> {
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

  impl MailboxRuntime for TestMailboxRuntime {
    type Queue<M>
      = TestQueue<M>
    where
      M: Element;
    type Signal = TestSignal;

    fn build_mailbox<M>(&self, options: MailboxOptions) -> MailboxPair<Self::Queue<M>, Self::Signal>
    where
      M: Element, {
      let capacity = self.resolve_capacity(options);
      let queue = MpscQueue::new(RcBackendHandle::new(capacity));
      let signal = TestSignal::default();
      let mailbox = QueueMailbox::new(queue, signal);
      let sender = mailbox.producer();
      (mailbox, sender)
    }
  }

  #[cfg(test)]
  mod tests {
    use super::*;
    use core::pin::Pin;
    use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

    #[test]
    fn test_mailbox_runtime_delivers_fifo() {
      let runtime = TestMailboxRuntime::with_capacity_per_queue(2);
      let _ = TestMailboxRuntime::unbounded();
      let (mailbox, sender) = runtime.build_default_mailbox::<u32>();

      sender.try_send(1).unwrap();
      sender.try_send(2).unwrap();

      let mut future = mailbox.recv();
      let waker = noop_waker();
      let mut cx = Context::from_waker(&waker);
      let mut pinned = unsafe { Pin::new_unchecked(&mut future) };

      assert_eq!(pinned.as_mut().poll(&mut cx), Poll::Ready(1));
      assert_eq!(pinned.poll(&mut cx), Poll::Ready(2));
    }

    fn noop_waker() -> Waker {
      unsafe { Waker::from_raw(noop_raw_waker()) }
    }

    fn noop_raw_waker() -> RawWaker {
      fn clone(_: *const ()) -> RawWaker {
        noop_raw_waker()
      }
      fn wake(_: *const ()) {}
      fn wake_by_ref(_: *const ()) {}
      fn drop(_: *const ()) {}

      RawWaker::new(core::ptr::null(), &RawWakerVTable::new(clone, wake, wake_by_ref, drop))
    }
  }
}
