use crate::actor::actor::props::{ContextDecorator, ReceiverMiddleware, SenderMiddleware, SpawnFunc, SpawnMiddleware};
use crate::actor::message::{ContextDecoratorFunc, ReceiverFunc, SenderFunc};

pub fn make_receiver_middleware_chain(
  receiver_middleware: &[ReceiverMiddleware],
  last_receiver: ReceiverFunc,
) -> Option<ReceiverFunc> {
  if receiver_middleware.is_empty() {
    tracing::debug!("make_receiver_middleware_chain: receiver_middleware is empty");
    return None;
  }

  let mut h = receiver_middleware.last().unwrap().run(last_receiver);
  for middleware in receiver_middleware.iter().rev().skip(1) {
    tracing::debug!("+");
    h = middleware.run(h);
  }

  tracing::debug!("make_receiver_middleware_chain: receiver_middleware is not empty");
  Some(h)
}

// SenderMiddlewareChain
pub fn make_sender_middleware_chain(
  sender_middleware: &[SenderMiddleware],
  last_sender: SenderFunc,
) -> Option<SenderFunc> {
  if sender_middleware.is_empty() {
    return None;
  }

  let mut h = sender_middleware.last().unwrap().run(last_sender);
  for middleware in sender_middleware.iter().rev().skip(1) {
    h = middleware.run(h);
  }

  Some(h)
}

// ContextDecoratorChain
pub fn make_context_decorator_chain(
  decorator: &[ContextDecorator],
  last_decorator: ContextDecoratorFunc,
) -> Option<ContextDecoratorFunc> {
  if decorator.is_empty() {
    return None;
  }

  let mut h = decorator.last().unwrap().run(last_decorator);
  for d in decorator.iter().rev().skip(1) {
    h = d.run(h);
  }

  Some(h)
}

// SpawnMiddlewareChain
pub fn make_spawn_middleware_chain(spawn_middleware: &[SpawnMiddleware], last_spawn: SpawnFunc) -> Option<SpawnFunc> {
  if spawn_middleware.is_empty() {
    return None;
  }

  let mut h = spawn_middleware.last().unwrap().run(last_spawn);
  for middleware in spawn_middleware.iter().rev().skip(1) {
    h = middleware.run(h);
  }

  Some(h)
}
