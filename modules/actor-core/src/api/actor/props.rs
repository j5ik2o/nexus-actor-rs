use crate::runtime::context::ActorContext;
use crate::runtime::message::DynMessage;
use crate::runtime::system::InternalProps;
use crate::Supervisor;
use crate::SystemMessage;
use crate::{MailboxFactory, MailboxOptions, PriorityEnvelope};
use nexus_utils_core_rs::Element;

use super::behavior::SupervisorStrategyConfig;
use super::{ActorAdapter, Behavior, Context};
use crate::api::messaging::MessageEnvelope;
use core::marker::PhantomData;

pub struct Props<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone, {
  inner: InternalProps<DynMessage, R>,
  _marker: PhantomData<U>,
  supervisor: SupervisorStrategyConfig,
}

impl<U, R> Props<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
{
  pub fn new<F>(options: MailboxOptions, handler: F) -> Self
  where
    F: for<'r, 'ctx> FnMut(&mut Context<'r, 'ctx, U, R>, U) + 'static, {
    Self::with_behavior(options, Behavior::stateless(handler))
  }

  pub fn with_behavior(options: MailboxOptions, behavior: Behavior<U, R>) -> Self {
    Self::with_behavior_and_system::<fn(&mut Context<'_, '_, U, R>, SystemMessage)>(options, behavior, None)
  }

  pub fn with_system_handler<F, G>(options: MailboxOptions, user_handler: F, system_handler: Option<G>) -> Self
  where
    F: for<'r, 'ctx> FnMut(&mut Context<'r, 'ctx, U, R>, U) + 'static,
    G: for<'r, 'ctx> FnMut(&mut Context<'r, 'ctx, U, R>, SystemMessage) + 'static, {
    let behavior = Behavior::stateless(user_handler);
    Self::with_behavior_and_system(options, behavior, system_handler)
  }

  pub fn with_behavior_and_system<S>(
    options: MailboxOptions,
    behavior: Behavior<U, R>,
    system_handler: Option<S>,
  ) -> Self
  where
    S: for<'r, 'ctx> FnMut(&mut Context<'r, 'ctx, U, R>, SystemMessage) + 'static, {
    let mut adapter = ActorAdapter::new(behavior, system_handler);
    let map_system = ActorAdapter::<U, R>::create_map_system();
    let supervisor = adapter.supervisor_config();

    let handler = move |ctx: &mut ActorContext<'_, DynMessage, R, dyn Supervisor<DynMessage>>, message: DynMessage| {
      let Ok(envelope) = message.downcast::<MessageEnvelope<U>>() else {
        panic!("unexpected message type delivered to typed handler");
      };
      match envelope {
        MessageEnvelope::User(message) => {
          let mut typed_ctx = Context::new(ctx);
          adapter.handle_user(&mut typed_ctx, message);
        }
        MessageEnvelope::System(message) => {
          let mut typed_ctx = Context::new(ctx);
          adapter.handle_system(&mut typed_ctx, message);
        }
      }
    };

    let inner = InternalProps::new(options, map_system, handler);
    Self {
      inner,
      _marker: PhantomData,
      supervisor,
    }
  }

  pub(crate) fn into_parts(self) -> (InternalProps<DynMessage, R>, SupervisorStrategyConfig) {
    (self.inner, self.supervisor)
  }
}
