use crate::runtime::context::ActorContext;
use crate::runtime::system::InternalProps;
use crate::supervisor::Supervisor;
use crate::SystemMessage;
use crate::{MailboxOptions, MailboxRuntime, PriorityEnvelope};
use nexus_utils_core_rs::Element;

use super::{ActorAdapter, Behavior, Context};
use crate::api::messaging::MessageEnvelope;

pub struct Props<U, R>
where
  U: Element,
  R: MailboxRuntime + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone, {
  inner: InternalProps<MessageEnvelope<U>, R>,
}

impl<U, R> Props<U, R>
where
  U: Element,
  R: MailboxRuntime + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone,
{
  pub fn new<F>(options: MailboxOptions, handler: F) -> Self
  where
    F: FnMut(&mut ActorContext<'_, MessageEnvelope<U>, R, dyn Supervisor<MessageEnvelope<U>>>, U) + 'static, {
    Self::with_system_handler(
      options,
      handler,
      Option::<
        fn(
          &mut ActorContext<'_, MessageEnvelope<U>, R, dyn Supervisor<MessageEnvelope<U>>>,
          SystemMessage,
        ),
      >::None,
    )
  }

  pub fn from_typed_handler<G>(options: MailboxOptions, handler: G) -> Self
  where
    G: for<'r, 'ctx> FnMut(&mut Context<'r, 'ctx, U, R>, U) + 'static, {
    Self::with_behavior(options, Behavior::stateless(handler))
  }

  pub fn with_behavior(options: MailboxOptions, behavior: Behavior<U, R>) -> Self {
    Self::with_behavior_and_system::<
      fn(&mut ActorContext<'_, MessageEnvelope<U>, R, dyn Supervisor<MessageEnvelope<U>>>, SystemMessage),
    >(options, behavior, None)
  }

  pub fn with_system_handler<F, G>(options: MailboxOptions, user_handler: F, system_handler: Option<G>) -> Self
  where
    F: FnMut(&mut ActorContext<'_, MessageEnvelope<U>, R, dyn Supervisor<MessageEnvelope<U>>>, U) + 'static,
    G: for<'ctx> FnMut(&mut ActorContext<'ctx, MessageEnvelope<U>, R, dyn Supervisor<MessageEnvelope<U>>>, SystemMessage)
      + 'static, {
    let mut user_handler = user_handler;
    let behavior = Behavior::stateless(move |ctx: &mut Context<'_, '_, U, R>, message| {
      user_handler(ctx.inner(), message);
    });
    Self::with_behavior_and_system(options, behavior, system_handler)
  }

  pub fn with_behavior_and_system<S>(
    options: MailboxOptions,
    behavior: Behavior<U, R>,
    system_handler: Option<S>,
  ) -> Self
  where
    S: for<'ctx> FnMut(&mut ActorContext<'ctx, MessageEnvelope<U>, R, dyn Supervisor<MessageEnvelope<U>>>, SystemMessage)
      + 'static, {
    let mut adapter = ActorAdapter::new(behavior, system_handler);
    let map_system = ActorAdapter::<U, R>::create_map_system();

    let handler = move |ctx: &mut ActorContext<'_, MessageEnvelope<U>, R, dyn Supervisor<MessageEnvelope<U>>>,
                        envelope: MessageEnvelope<U>| {
      match envelope {
        MessageEnvelope::User(message) => {
          adapter.handle_user(ctx, message);
        }
        MessageEnvelope::System(message) => {
          adapter.handle_system(ctx, message);
        }
      }
    };

    let inner = InternalProps::new(options, map_system, handler);
    Self { inner }
  }

  pub(crate) fn into_inner(self) -> InternalProps<MessageEnvelope<U>, R> {
    self.inner
  }
}
