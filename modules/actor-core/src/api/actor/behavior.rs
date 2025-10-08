use alloc::boxed::Box;
use alloc::sync::Arc;

use crate::api::messaging::MessageEnvelope;
use crate::runtime::context::{ActorContext, MapSystemFn};
use crate::MailboxFactory;
use crate::PriorityEnvelope;
use crate::Supervisor;
use crate::SystemMessage;
use nexus_utils_core_rs::Element;

use super::Context;

type BehaviorFn<U, R> = dyn for<'r, 'ctx> FnMut(&mut Context<'r, 'ctx, U, R>, U) + 'static;
type SystemHandlerFn<U, R> = dyn for<'ctx> FnMut(&mut ActorContext<'ctx, MessageEnvelope<U>, R, dyn Supervisor<MessageEnvelope<U>>>, SystemMessage)
  + 'static;

/// Minimal typed behavior abstraction.
pub struct Behavior<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone, {
  pub(super) handler: Box<BehaviorFn<U, R>>,
}

impl<U, R> Behavior<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone,
{
  pub fn stateless<F>(handler: F) -> Self
  where
    F: for<'r, 'ctx> FnMut(&mut Context<'r, 'ctx, U, R>, U) + 'static, {
    Self {
      handler: Box::new(handler),
    }
  }
}

/// Adapter bridging Behavior and ActorContext, managing map_system generation.
pub struct ActorAdapter<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone, {
  pub(super) behavior: Behavior<U, R>,
  pub(super) system_handler: Option<Box<SystemHandlerFn<U, R>>>,
}

impl<U, R> ActorAdapter<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone,
{
  pub fn new<S>(behavior: Behavior<U, R>, system_handler: Option<S>) -> Self
  where
    S: for<'ctx> FnMut(&mut ActorContext<'ctx, MessageEnvelope<U>, R, dyn Supervisor<MessageEnvelope<U>>>, SystemMessage)
      + 'static, {
    Self {
      behavior,
      system_handler: system_handler.map(|h| Box::new(h) as Box<SystemHandlerFn<U, R>>),
    }
  }

  pub fn handle_user(
    &mut self,
    ctx: &mut ActorContext<'_, MessageEnvelope<U>, R, dyn Supervisor<MessageEnvelope<U>>>,
    message: U,
  ) {
    let mut typed_ctx = Context::new(ctx);
    (self.behavior.handler)(&mut typed_ctx, message);
  }

  pub fn handle_system(
    &mut self,
    ctx: &mut ActorContext<'_, MessageEnvelope<U>, R, dyn Supervisor<MessageEnvelope<U>>>,
    message: SystemMessage,
  ) {
    if let Some(handler) = self.system_handler.as_mut() {
      handler(ctx, message);
    }
  }

  /// Creates map_system closure for Guardian/Scheduler integration.
  /// Current implementation wraps SystemMessage in MessageEnvelope::System.
  /// Future extension point: allow user-defined enum mapping.
  pub fn create_map_system() -> Arc<MapSystemFn<MessageEnvelope<U>>> {
    Arc::new(MessageEnvelope::System)
  }
}
