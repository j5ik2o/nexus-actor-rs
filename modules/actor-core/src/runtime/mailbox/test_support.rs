use alloc::rc::Rc;
use core::cell::RefCell;
use core::fmt;
use core::future::Future;
use core::marker::PhantomData;
use core::pin::Pin;
use core::task::{Context, Poll};

use nexus_utils_core_rs::{Element, MpscBuffer, MpscHandle, MpscQueue, QueueSize, RingBufferBackend, Shared};

use super::queue_mailbox::{MailboxOptions, QueueMailbox};
use super::traits::{MailboxFactory, MailboxPair, MailboxSignal};

#[derive(Clone, Debug, Default)]
pub struct TestMailboxFactory {
  capacity: Option<usize>,
}

impl TestMailboxFactory {
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

impl MailboxFactory for TestMailboxFactory {
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

  use super::super::traits::Mailbox;

  #[test]
  fn test_mailbox_runtime_delivers_fifo() {
    let runtime = TestMailboxFactory::with_capacity_per_queue(2);
    let _ = TestMailboxFactory::unbounded();
    let (mailbox, sender) = runtime.build_default_mailbox::<u32>();

    sender.try_send(1).unwrap();
    sender.try_send(2).unwrap();

    let mut future = mailbox.recv();
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    let mut pinned = unsafe { Pin::new_unchecked(&mut future) };

    assert_eq!(pinned.as_mut().poll(&mut cx), Poll::Ready(Ok(1)));
    assert_eq!(pinned.poll(&mut cx), Poll::Ready(Ok(2)));
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
