use alloc::sync::Arc;
use core::cell::UnsafeCell;
use core::fmt;
use core::future::Future;
use core::pin::Pin;
use core::sync::atomic::{AtomicU8, Ordering};
use core::task::{Context, Poll};

use crate::api::{InternalMessageDispatcher, MessageDispatcher, MessageEnvelope, MessageMetadata};
use crate::runtime::message::DynMessage;
use crate::PriorityEnvelope;
use futures::task::AtomicWaker;
use nexus_utils_core_rs::{Element, QueueError};

pub type AskResult<T> = Result<T, AskError>;

#[derive(Debug)]
pub enum AskError {
  MissingResponder,
  SendFailed(QueueError<PriorityEnvelope<DynMessage>>),
  ResponderDropped,
  ResponseAwaitCancelled,
  Timeout,
}

impl fmt::Display for AskError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      AskError::MissingResponder => write!(f, "no responder available for current message"),
      AskError::SendFailed(err) => write!(f, "failed to send ask response: {:?}", err),
      AskError::ResponderDropped => write!(f, "ask responder dropped before sending a response"),
      AskError::ResponseAwaitCancelled => write!(f, "ask future was cancelled before completion"),
      AskError::Timeout => write!(f, "ask future timed out"),
    }
  }
}

impl From<QueueError<PriorityEnvelope<DynMessage>>> for AskError {
  fn from(value: QueueError<PriorityEnvelope<DynMessage>>) -> Self {
    AskError::SendFailed(value)
  }
}

const STATE_PENDING: u8 = 0;
const STATE_READY: u8 = 1;
const STATE_CANCELLED: u8 = 2;
const STATE_RESPONDER_DROPPED: u8 = 3;

struct AskShared<Resp> {
  state: AtomicU8,
  value: UnsafeCell<Option<Resp>>,
  waker: AtomicWaker,
}

impl<Resp> AskShared<Resp> {
  fn new() -> Self {
    Self {
      state: AtomicU8::new(STATE_PENDING),
      value: UnsafeCell::new(None),
      waker: AtomicWaker::new(),
    }
  }

  fn complete(&self, value: Resp) -> bool {
    match self
      .state
      .compare_exchange(STATE_PENDING, STATE_READY, Ordering::AcqRel, Ordering::Acquire)
    {
      Ok(_) => {
        unsafe {
          *self.value.get() = Some(value);
        }
        self.waker.wake();
        true
      }
      Err(_) => false,
    }
  }

  fn cancel(&self) -> bool {
    self
      .state
      .compare_exchange(STATE_PENDING, STATE_CANCELLED, Ordering::AcqRel, Ordering::Acquire)
      .map(|_| {
        self.waker.wake();
      })
      .is_ok()
  }

  fn responder_dropped(&self) {
    if self
      .state
      .compare_exchange(
        STATE_PENDING,
        STATE_RESPONDER_DROPPED,
        Ordering::AcqRel,
        Ordering::Acquire,
      )
      .is_ok()
    {
      self.waker.wake();
    }
  }

  unsafe fn take_value(&self) -> Option<Resp> {
    (*self.value.get()).take()
  }

  fn state(&self) -> u8 {
    self.state.load(Ordering::Acquire)
  }
}

unsafe impl<Resp: Send> Send for AskShared<Resp> {}
unsafe impl<Resp: Send> Sync for AskShared<Resp> {}

pub struct AskFuture<Resp> {
  shared: Arc<AskShared<Resp>>,
}

impl<Resp> AskFuture<Resp> {
  fn new(shared: Arc<AskShared<Resp>>) -> Self {
    Self { shared }
  }
}

impl<Resp> Future for AskFuture<Resp> {
  type Output = AskResult<Resp>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let shared = &self.shared;

    loop {
      match shared.state() {
        STATE_READY => {
          let value = unsafe { shared.take_value() }.expect("ask future missing value");
          return Poll::Ready(Ok(value));
        }
        STATE_RESPONDER_DROPPED => return Poll::Ready(Err(AskError::ResponderDropped)),
        STATE_CANCELLED => return Poll::Ready(Err(AskError::ResponseAwaitCancelled)),
        STATE_PENDING => {
          shared.waker.register(cx.waker());
          if shared.state() == STATE_PENDING {
            return Poll::Pending;
          }
        }
        _ => return Poll::Ready(Err(AskError::ResponseAwaitCancelled)),
      }
    }
  }
}

impl<Resp> Drop for AskFuture<Resp> {
  fn drop(&mut self) {
    let _ = self.shared.cancel();
  }
}

unsafe impl<Resp> Send for AskFuture<Resp> where Resp: Send {}
unsafe impl<Resp> Sync for AskFuture<Resp> where Resp: Send {}
impl<Resp> Unpin for AskFuture<Resp> {}

pub struct AskTimeoutFuture<Resp, TFut> {
  ask: Option<AskFuture<Resp>>,
  timeout: Option<TFut>,
}

impl<Resp, TFut> AskTimeoutFuture<Resp, TFut> {
  fn new(ask: AskFuture<Resp>, timeout: TFut) -> Self {
    Self {
      ask: Some(ask),
      timeout: Some(timeout),
    }
  }
}

impl<Resp, TFut> Future for AskTimeoutFuture<Resp, TFut>
where
  TFut: Future<Output = ()> + Unpin,
  Resp: Element,
{
  type Output = AskResult<Resp>;

  fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    if let Some(ask) = self.ask.as_mut() {
      match Pin::new(ask).poll(cx) {
        Poll::Ready(result) => {
          self.ask = None;
          self.timeout = None;
          return Poll::Ready(result);
        }
        Poll::Pending => {}
      }
    }

    if let Some(timeout) = self.timeout.as_mut() {
      if Pin::new(timeout).poll(cx).is_ready() {
        self.timeout = None;
        self.ask.take();
        return Poll::Ready(Err(AskError::Timeout));
      }
    }

    Poll::Pending
  }
}

impl<Resp, TFut> Drop for AskTimeoutFuture<Resp, TFut> {
  fn drop(&mut self) {
    if self.timeout.is_some() {
      self.ask.take();
    }
  }
}

pub fn ask_with_timeout<Resp, TFut>(future: AskFuture<Resp>, timeout: TFut) -> AskTimeoutFuture<Resp, TFut>
where
  TFut: Future<Output = ()> + Unpin,
  Resp: Element, {
  AskTimeoutFuture::new(future, timeout)
}

pub(crate) fn create_ask_handles<Resp>() -> (AskFuture<Resp>, MessageDispatcher<Resp>)
where
  Resp: Element, {
  let shared = Arc::new(AskShared::<Resp>::new());
  let future = AskFuture::new(shared.clone());
  let dispatch_state = shared.clone();
  let drop_state = shared.clone();

  let dispatch = Arc::new(move |message: DynMessage, _priority: i8| {
    let envelope = message
      .downcast::<MessageEnvelope<Resp>>()
      .unwrap_or_else(|_| panic!("ask responder received mismatched message type"));
    match envelope {
      MessageEnvelope::User(user) => {
        let (value, _metadata) = user.into_parts();
        if !dispatch_state.complete(value) {
          // 応答先がキャンセル済みの場合は値を破棄
        }
      }
      MessageEnvelope::System(_) => {
        panic!("ask responder received system message");
      }
    }
    Ok(())
  });

  let drop_hook = Arc::new(move || {
    drop_state.responder_dropped();
  });

  let internal = InternalMessageDispatcher::with_drop_hook(dispatch, drop_hook);
  let responder = MessageDispatcher::new(internal);
  (future, responder)
}

pub(crate) fn dispatch_response<Resp>(metadata: &MessageMetadata, message: Resp) -> AskResult<()>
where
  Resp: Element, {
  let dispatcher = metadata
    .responder_as::<Resp>()
    .or_else(|| metadata.sender_as::<Resp>())
    .ok_or(AskError::MissingResponder)?;
  dispatcher.dispatch_user(message).map_err(AskError::from)
}
