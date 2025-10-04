use crate::actor::core::context_decorator::ContextDecorator;
use crate::actor::core::context_decorator_chain::ContextDecoratorChain;
use crate::actor::core::receiver_middleware::ReceiverMiddleware;
use crate::actor::core::receiver_middleware_chain::ReceiverMiddlewareChain;
use crate::actor::core::sender_middleware::SenderMiddleware;
use crate::actor::core::sender_middleware_chain::SenderMiddlewareChain;
use crate::actor::core::spawn_middleware::SpawnMiddleware;
use crate::actor::core::spawner::Spawner;
use nexus_actor_core_rs::context::{compose_receiver_chain, compose_sender_chain};

pub fn make_receiver_middleware_chain(
  receiver_middleware: &[ReceiverMiddleware],
  last_receiver: ReceiverMiddlewareChain,
) -> Option<ReceiverMiddlewareChain> {
  if receiver_middleware.is_empty() {
    return None;
  }

  compose_receiver_chain(
    receiver_middleware.iter().map(|middleware| middleware.as_core()),
    last_receiver.into_core(),
  )
  .map(ReceiverMiddlewareChain::from_core)
}

pub fn make_sender_middleware_chain(
  sender_middleware: &[SenderMiddleware],
  last_sender: SenderMiddlewareChain,
) -> Option<SenderMiddlewareChain> {
  if sender_middleware.is_empty() {
    return None;
  }

  compose_sender_chain(
    sender_middleware.iter().map(|middleware| middleware.as_core()),
    last_sender.into_core(),
  )
  .map(SenderMiddlewareChain::from_core)
}

pub fn make_context_decorator_chain(
  decorator: &[ContextDecorator],
  last_decorator: ContextDecoratorChain,
) -> Option<ContextDecoratorChain> {
  if decorator.is_empty() {
    return None;
  }
  let mut chain = last_decorator;
  for d in decorator.iter().rev() {
    chain = chain.prepend(d.clone());
  }
  Some(chain)
}

pub fn make_spawn_middleware_chain(spawn_middleware: &[SpawnMiddleware], last_spawner: Spawner) -> Option<Spawner> {
  if spawn_middleware.is_empty() {
    return None;
  }

  let mut current = last_spawner;
  for middleware in spawn_middleware.iter().rev() {
    current = middleware.run(current);
  }

  Some(current)
}
