#[cfg(not(target_has_atomic = "ptr"))]
use alloc::rc::Rc as Arc;
#[cfg(target_has_atomic = "ptr")]
use alloc::sync::Arc;
use core::cell::UnsafeCell;
use core::fmt;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use crate::api::{InternalMessageSender, MessageEnvelope, MessageSender};
use crate::runtime::mailbox::traits::MailboxConcurrency;
use crate::runtime::message::{discard_metadata, DynMessage};
use crate::PriorityEnvelope;
use nexus_utils_core_rs::sync::ArcShared;
use nexus_utils_core_rs::{Element, QueueError};
use portable_atomic::{AtomicU8, Ordering};

#[cfg(target_has_atomic = "ptr")]
use futures::task::AtomicWaker;

#[cfg(not(target_has_atomic = "ptr"))]
mod local_waker {
  use core::cell::RefCell;
  use core::task::Waker;

  /// Single-threaded waker used on targets without atomic pointer support.
  #[derive(Default)]
  pub struct LocalWaker {
    stored: RefCell<Option<Waker>>,
  }

  impl LocalWaker {
    pub fn new() -> Self {
      Self::default()
    }

    pub fn register(&self, waker: &Waker) {
      *self.stored.borrow_mut() = Some(waker.clone());
    }

    pub fn wake(&self) {
      if let Some(waker) = self.stored.borrow_mut().take() {
        waker.wake();
      }
    }
  }
}

#[cfg(target_has_atomic = "ptr")]
type SharedWaker = AtomicWaker;

#[cfg(not(target_has_atomic = "ptr"))]
type SharedWaker = local_waker::LocalWaker;

#[cfg(target_has_atomic = "ptr")]
type DispatchFn = dyn Fn(DynMessage, i8) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> + Send + Sync;

#[cfg(not(target_has_atomic = "ptr"))]
type DispatchFn = dyn Fn(DynMessage, i8) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>>;

#[cfg(target_has_atomic = "ptr")]
type DropHookFn = dyn Fn() + Send + Sync;

#[cfg(not(target_has_atomic = "ptr"))]
type DropHookFn = dyn Fn();

/// Type alias representing the result of an `ask` operation as `Result`.
pub type AskResult<T> = Result<T, AskError>;

/// Errors that can occur during `ask` processing.
#[derive(Debug)]
pub enum AskError {
  /// Responder not found
  MissingResponder,
  /// Message send failed
  SendFailed(QueueError<PriorityEnvelope<DynMessage>>),
  /// Responder was dropped before responding
  ResponderDropped,
  /// Response await was cancelled
  ResponseAwaitCancelled,
  /// Timeout occurred
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

/// Internal state shared between `AskFuture` and responder.
struct AskShared<Resp> {
  state: AtomicU8,
  value: UnsafeCell<Option<Resp>>,
  waker: SharedWaker,
}

impl<Resp> AskShared<Resp> {
  fn new() -> Self {
    Self {
      state: AtomicU8::new(STATE_PENDING),
      value: UnsafeCell::new(None),
      waker: SharedWaker::new(),
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
    // SAFETY: This function is unsafe and the caller is responsible for guaranteeing exclusive access
    unsafe { (*self.value.get()).take() }
  }

  fn state(&self) -> u8 {
    self.state.load(Ordering::Acquire)
  }
}

unsafe impl<Resp: Send> Send for AskShared<Resp> {}
unsafe impl<Resp: Send> Sync for AskShared<Resp> {}

/// Future that awaits a response from an `ask` operation.
///
/// Future returned when sending a message with the `ask` pattern.
/// Waits until a response message arrives and returns the result.
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

/// `AskFuture` wrapper with timeout control.
///
/// Adds timeout functionality to `AskFuture`.
/// Returns `AskError::Timeout` if timeout completes first.
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

/// Helper function to create an `AskFuture` with timeout.
///
/// # Arguments
/// * `future` - Base `AskFuture`
/// * `timeout` - Future for timeout control
///
/// # Returns
/// `AskTimeoutFuture` with timeout control
pub fn ask_with_timeout<Resp, TFut>(future: AskFuture<Resp>, timeout: TFut) -> AskTimeoutFuture<Resp, TFut>
where
  TFut: Future<Output = ()> + Unpin,
  Resp: Element, {
  AskTimeoutFuture::new(future, timeout)
}

/// Creates a Future and responder pair for the `ask` pattern (internal API).
///
/// # Returns
/// Tuple of (`AskFuture`, `MessageSender`)
pub(crate) fn create_ask_handles<Resp, C>() -> (AskFuture<Resp>, MessageSender<Resp, C>)
where
  Resp: Element,
  C: MailboxConcurrency, {
  let shared = Arc::new(AskShared::<Resp>::new());
  let future = AskFuture::new(shared.clone());
  let dispatch_state = shared.clone();
  let drop_state = shared.clone();

  let dispatch_impl: Arc<DispatchFn> = Arc::new(move |message: DynMessage, _priority: i8| {
    let envelope = message
      .downcast::<MessageEnvelope<Resp>>()
      .unwrap_or_else(|_| panic!("ask responder received mismatched message type"));
    match envelope {
      MessageEnvelope::User(user) => {
        let (value, metadata_key) = user.into_parts();
        if let Some(key) = metadata_key {
          discard_metadata(key);
        }
        if !dispatch_state.complete(value) {
          // Discard value if response destination was already cancelled
        }
      }
      MessageEnvelope::System(_) => {
        panic!("ask responder received system message");
      }
    }
    Ok(())
  });
  let dispatch = ArcShared::from_arc(dispatch_impl);

  let drop_hook_impl: Arc<DropHookFn> = Arc::new(move || {
    drop_state.responder_dropped();
  });
  let drop_hook = ArcShared::from_arc(drop_hook_impl);

  let internal = InternalMessageSender::<C>::with_drop_hook(dispatch, drop_hook);
  let responder = MessageSender::new(internal);
  (future, responder)
}
