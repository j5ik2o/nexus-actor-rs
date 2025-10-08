use super::ask::create_ask_handles;
use super::{ask_with_timeout, AskError};
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use futures::future;

fn noop_waker() -> Waker {
  fn clone(_: *const ()) -> RawWaker {
    noop_raw_waker()
  }
  fn wake(_: *const ()) {}
  fn wake_by_ref(_: *const ()) {}
  fn drop(_: *const ()) {}

  fn noop_raw_waker() -> RawWaker {
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);
    RawWaker::new(core::ptr::null(), &VTABLE)
  }

  unsafe { Waker::from_raw(noop_raw_waker()) }
}

fn resolve<F>(mut future: F) -> F::Output
where
  F: Future + Unpin, {
  let waker = noop_waker();
  let mut future = Pin::new(&mut future);
  let mut cx = Context::from_waker(&waker);
  loop {
    match future.as_mut().poll(&mut cx) {
      Poll::Ready(value) => return value,
      Poll::Pending => core::hint::spin_loop(),
    }
  }
}

#[test]
fn ask_future_completes_successfully() {
  let (future, responder) = create_ask_handles::<u32>();
  responder.dispatch_user(7_u32).expect("dispatch succeeds");

  let result = resolve(future);
  assert_eq!(result.expect("ask result"), 7);
}

#[test]
fn ask_future_timeout_returns_error() {
  let (future, _responder) = create_ask_handles::<u32>();
  let timed = ask_with_timeout(future, future::ready(()));

  let result = resolve(timed);
  assert!(
    matches!(result, Err(AskError::Timeout)),
    "unexpected result: {:?}",
    result
  );
}

#[test]
fn ask_future_responder_drop_propagates() {
  let (future, responder) = create_ask_handles::<u32>();
  drop(responder);

  let result = resolve(future);
  assert!(matches!(result, Err(AskError::ResponderDropped)));
}

#[test]
fn ask_future_cancelled_on_drop() {
  let (future, responder) = create_ask_handles::<u32>();
  drop(future);
  drop(responder);
}
