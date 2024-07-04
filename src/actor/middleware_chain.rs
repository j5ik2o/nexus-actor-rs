use crate::actor::actor::context_decorator_chain_func::ContextDecoratorChainFunc;
use crate::actor::actor::context_decorator_func::ContextDecoratorFunc;
use crate::actor::actor::receiver_middleware_chain_func::ReceiverMiddlewareChainFunc;
use crate::actor::actor::receiver_middleware_func::ReceiverMiddlewareFunc;
use crate::actor::actor::sender_middleware_chain_func::SenderMiddlewareChainFunc;
use crate::actor::actor::sender_middleware_func::SenderMiddlewareFunc;
use crate::actor::actor::spawn_func::SpawnFunc;
use crate::actor::actor::spawn_middleware_func::SpawnMiddlewareFunc;

pub fn make_receiver_middleware_chain(
  receiver_middleware: &[ReceiverMiddlewareFunc],
  last_receiver: ReceiverMiddlewareChainFunc,
) -> Option<ReceiverMiddlewareChainFunc> {
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
  sender_middleware: &[SenderMiddlewareFunc],
  last_sender: SenderMiddlewareChainFunc,
) -> Option<SenderMiddlewareChainFunc> {
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
  decorator: &[ContextDecoratorFunc],
  last_decorator: ContextDecoratorChainFunc,
) -> Option<ContextDecoratorChainFunc> {
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
pub fn make_spawn_middleware_chain(
  spawn_middleware: &[SpawnMiddlewareFunc],
  last_spawn: SpawnFunc,
) -> Option<SpawnFunc> {
  if spawn_middleware.is_empty() {
    return None;
  }

  let mut h = spawn_middleware.last().unwrap().run(last_spawn);
  for middleware in spawn_middleware.iter().rev().skip(1) {
    h = middleware.run(h);
  }

  Some(h)
}
