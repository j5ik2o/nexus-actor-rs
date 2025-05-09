use crate::actor::core::context_decorator::ContextDecorator;
use crate::actor::core::context_decorator_chain::ContextDecoratorChain;
use crate::actor::core::receiver_middleware::ReceiverMiddleware;
use crate::actor::core::receiver_middleware_chain::ReceiverMiddlewareChain;
use crate::actor::core::sender_middleware::SenderMiddleware;
use crate::actor::core::sender_middleware_chain::SenderMiddlewareChain;
use crate::actor::core::spawn_middleware::SpawnMiddleware;
use crate::actor::core::spawner::Spawner;

pub fn make_receiver_middleware_chain(
  receiver_middleware: &[ReceiverMiddleware],
  last_receiver: ReceiverMiddlewareChain,
) -> Option<ReceiverMiddlewareChain> {
  if receiver_middleware.is_empty() {
    return None;
  }
  let mut h = receiver_middleware.last().unwrap().run(last_receiver);
  for middleware in receiver_middleware.iter().rev().skip(1) {
    h = middleware.run(h);
  }
  Some(h)
}

pub fn make_sender_middleware_chain(
  sender_middleware: &[SenderMiddleware],
  last_sender: SenderMiddlewareChain,
) -> Option<SenderMiddlewareChain> {
  if sender_middleware.is_empty() {
    return None;
  }
  let mut h = sender_middleware.last().unwrap().run(last_sender);
  for middleware in sender_middleware.iter().rev().skip(1) {
    h = middleware.run(h);
  }
  Some(h)
}

pub fn make_context_decorator_chain(
  decorator: &[ContextDecorator],
  last_decorator: ContextDecoratorChain,
) -> Option<ContextDecoratorChain> {
  if decorator.is_empty() {
    return None;
  }
  let mut h = decorator.last().unwrap().run(last_decorator);
  for d in decorator.iter().rev().skip(1) {
    h = d.run(h);
  }
  Some(h)
}

pub fn make_spawn_middleware_chain(spawn_middleware: &[SpawnMiddleware], last_spawner: Spawner) -> Option<Spawner> {
  if spawn_middleware.is_empty() {
    return None;
  }
  let mut h = spawn_middleware.last().unwrap().run(last_spawner);
  for middleware in spawn_middleware.iter().rev().skip(1) {
    h = middleware.run(h);
  }
  Some(h)
}
