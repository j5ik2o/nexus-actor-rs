use alloc::sync::Arc;

use crate::context::{ActorContext, PriorityActorRef};
use crate::guardian::AlwaysRestart;
use crate::mailbox::SystemMessage;
use crate::supervisor::Supervisor;
use crate::system::{ActorSystem, Props, RootContext};
use crate::{MailboxOptions, MailboxRuntime, PriorityEnvelope};
use nexus_utils_core_rs::{Element, QueueError, DEFAULT_PRIORITY};

/// Typed envelope combining user and system messages.
#[derive(Debug, Clone)]
pub enum MessageEnvelope<U> {
  User(U),
  System(SystemMessage),
}

impl<U> Element for MessageEnvelope<U> where U: Element {}

pub struct TypedProps<U, R>
where
  U: Element,
  R: MailboxRuntime + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone, {
  inner: Props<MessageEnvelope<U>, R>,
}

impl<U, R> TypedProps<U, R>
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
      Option::<fn(&mut ActorContext<'_, MessageEnvelope<U>, R, dyn Supervisor<MessageEnvelope<U>>>, SystemMessage)>::None,
    )
  }

  pub fn with_system_handler<F, G>(options: MailboxOptions, user_handler: F, system_handler: Option<G>) -> Self
  where
    F: FnMut(&mut ActorContext<'_, MessageEnvelope<U>, R, dyn Supervisor<MessageEnvelope<U>>>, U) + 'static,
    G: FnMut(&mut ActorContext<'_, MessageEnvelope<U>, R, dyn Supervisor<MessageEnvelope<U>>>, SystemMessage) + 'static,
  {
    let map_system = Arc::new(|sys: SystemMessage| MessageEnvelope::System(sys));
    let mut user_handler = user_handler;
    let mut system_handler = system_handler;

    let handler = move |ctx: &mut ActorContext<'_, MessageEnvelope<U>, R, dyn Supervisor<MessageEnvelope<U>>>,
                        envelope: MessageEnvelope<U>| {
      match envelope {
        MessageEnvelope::User(message) => {
          user_handler(ctx, message);
        }
        MessageEnvelope::System(message) => {
          if let Some(ref mut handler) = system_handler {
            handler(ctx, message);
          }
        }
      }
    };

    let inner = Props::new(options, map_system, handler);
    Self { inner }
  }

  pub(crate) fn into_inner(self) -> Props<MessageEnvelope<U>, R> {
    self.inner
  }
}

#[derive(Clone)]
pub struct TypedActorRef<U, R>
where
  U: Element,
  R: MailboxRuntime + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone, {
  inner: PriorityActorRef<MessageEnvelope<U>, R>,
}

impl<U, R> TypedActorRef<U, R>
where
  U: Element,
  R: MailboxRuntime + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone,
{
  pub fn new(inner: PriorityActorRef<MessageEnvelope<U>, R>) -> Self {
    Self { inner }
  }

  pub fn tell(&self, message: U) -> Result<(), QueueError<PriorityEnvelope<MessageEnvelope<U>>>> {
    self
      .inner
      .try_send_with_priority(MessageEnvelope::User(message), DEFAULT_PRIORITY)
  }

  pub fn tell_with_priority(
    &self,
    message: U,
    priority: i8,
  ) -> Result<(), QueueError<PriorityEnvelope<MessageEnvelope<U>>>> {
    self
      .inner
      .try_send_with_priority(MessageEnvelope::User(message), priority)
  }

  pub fn send_system(&self, message: SystemMessage) -> Result<(), QueueError<PriorityEnvelope<MessageEnvelope<U>>>> {
    let priority = message.priority();
    self
      .inner
      .try_send_control_with_priority(MessageEnvelope::System(message), priority)
  }

  pub fn inner(&self) -> &PriorityActorRef<MessageEnvelope<U>, R> {
    &self.inner
  }
}

pub struct TypedActorSystem<U, R, Strat = AlwaysRestart>
where
  U: Element,
  R: MailboxRuntime + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone,
  Strat: crate::guardian::GuardianStrategy<MessageEnvelope<U>, R>, {
  inner: ActorSystem<MessageEnvelope<U>, R, Strat>,
}

impl<U, R> TypedActorSystem<U, R>
where
  U: Element,
  R: MailboxRuntime + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone,
{
  pub fn new(runtime: R) -> Self {
    Self {
      inner: ActorSystem::new(runtime),
    }
  }
}

impl<U, R, Strat> TypedActorSystem<U, R, Strat>
where
  U: Element,
  R: MailboxRuntime + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone,
  Strat: crate::guardian::GuardianStrategy<MessageEnvelope<U>, R>,
{
  pub fn root_context(&mut self) -> TypedRootContext<'_, U, R, Strat> {
    TypedRootContext {
      inner: self.inner.root_context(),
    }
  }

  pub fn inner(&mut self) -> &mut ActorSystem<MessageEnvelope<U>, R, Strat> {
    &mut self.inner
  }
}

pub struct TypedRootContext<'a, U, R, Strat>
where
  U: Element,
  R: MailboxRuntime + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone,
  Strat: crate::guardian::GuardianStrategy<MessageEnvelope<U>, R>, {
  inner: RootContext<'a, MessageEnvelope<U>, R, Strat>,
}

impl<'a, U, R, Strat> TypedRootContext<'a, U, R, Strat>
where
  U: Element,
  R: MailboxRuntime + Clone,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone,
  Strat: crate::guardian::GuardianStrategy<MessageEnvelope<U>, R>,
{
  pub fn spawn(
    &mut self,
    props: TypedProps<U, R>,
  ) -> Result<TypedActorRef<U, R>, QueueError<PriorityEnvelope<MessageEnvelope<U>>>> {
    let actor_ref = self.inner.spawn(props.into_inner())?;
    Ok(TypedActorRef::new(actor_ref))
  }

  pub fn dispatch_all(&mut self) -> Result<(), QueueError<PriorityEnvelope<MessageEnvelope<U>>>> {
    self.inner.dispatch_all()
  }

  pub fn raw(&mut self) -> &mut RootContext<'a, MessageEnvelope<U>, R, Strat> {
    &mut self.inner
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::mailbox::test_support::TestMailboxRuntime;
  use alloc::rc::Rc;
  use alloc::vec::Vec;
  use core::cell::RefCell;

  #[test]
  fn typed_actor_system_handles_user_messages() {
    let runtime = TestMailboxRuntime::unbounded();
    let mut system: TypedActorSystem<u32, _, AlwaysRestart> = TypedActorSystem::new(runtime);

    let log: Rc<RefCell<Vec<u32>>> = Rc::new(RefCell::new(Vec::new()));
    let log_clone = log.clone();

    let props = TypedProps::new(MailboxOptions::default(), move |_, msg: u32| {
      log_clone.borrow_mut().push(msg);
    });

    let mut root = system.root_context();
    let actor_ref = root.spawn(props).expect("spawn typed actor");
    actor_ref.tell(11).expect("tell");
    root.dispatch_all().expect("dispatch");

    assert_eq!(log.borrow().as_slice(), &[11]);
  }
}
