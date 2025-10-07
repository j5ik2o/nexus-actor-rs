use alloc::sync::Arc;
use core::convert::Infallible;

use crate::actor_id::ActorId;
use crate::actor_path::ActorPath;
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

/// Typed actor execution context wrapper.
/// 'r: lifetime of the mutable reference to ActorContext
/// 'ctx: lifetime parameter of ActorContext itself
pub struct TypedContext<'r, 'ctx, U, R>
where
  U: Element,
  R: MailboxRuntime + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone, {
  inner: &'r mut ActorContext<'ctx, MessageEnvelope<U>, R, dyn Supervisor<MessageEnvelope<U>>>,
}

impl<'r, 'ctx, U, R> TypedContext<'r, 'ctx, U, R>
where
  U: Element,
  R: MailboxRuntime + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone,
{
  pub fn actor_id(&self) -> ActorId {
    self.inner.actor_id()
  }

  pub fn actor_path(&self) -> &ActorPath {
    self.inner.actor_path()
  }

  pub fn watchers(&self) -> &[ActorId] {
    self.inner.watchers()
  }

  pub fn send_to_self(&self, message: U) -> Result<(), QueueError<PriorityEnvelope<MessageEnvelope<U>>>> {
    self
      .inner
      .send_to_self_with_priority(MessageEnvelope::User(message), DEFAULT_PRIORITY)
  }

  pub fn send_system_to_self(
    &self,
    message: SystemMessage,
  ) -> Result<(), QueueError<PriorityEnvelope<MessageEnvelope<U>>>> {
    let priority = message.priority();
    self
      .inner
      .send_control_to_self(MessageEnvelope::System(message), priority)
  }

  pub fn inner(&mut self) -> &mut ActorContext<'ctx, MessageEnvelope<U>, R, dyn Supervisor<MessageEnvelope<U>>> {
    self.inner
  }
}

/// Minimal typed behavior abstraction.
pub struct Behavior<U, R>
where
  U: Element,
  R: MailboxRuntime + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone, {
  handler: Box<dyn for<'r, 'ctx> FnMut(&mut TypedContext<'r, 'ctx, U, R>, U) + 'static>,
}

impl<U, R> Behavior<U, R>
where
  U: Element,
  R: MailboxRuntime + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone,
{
  pub fn stateless<F>(handler: F) -> Self
  where
    F: for<'r, 'ctx> FnMut(&mut TypedContext<'r, 'ctx, U, R>, U) + 'static, {
    Self {
      handler: Box::new(handler),
    }
  }
}

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
    G: for<'r, 'ctx> FnMut(&mut TypedContext<'r, 'ctx, U, R>, U) + 'static, {
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
    let behavior = Behavior::stateless(move |ctx: &mut TypedContext<'_, '_, U, R>, message| {
      user_handler(ctx.inner(), message);
    });
    Self::with_behavior_and_system(options, behavior, system_handler)
  }

  pub fn with_behavior_and_system<S>(
    options: MailboxOptions,
    mut behavior: Behavior<U, R>,
    mut system_handler: Option<S>,
  ) -> Self
  where
    S: for<'ctx> FnMut(&mut ActorContext<'ctx, MessageEnvelope<U>, R, dyn Supervisor<MessageEnvelope<U>>>, SystemMessage)
      + 'static, {
    let map_system = Arc::new(|sys: SystemMessage| MessageEnvelope::System(sys));
    let handler = move |ctx: &mut ActorContext<'_, MessageEnvelope<U>, R, dyn Supervisor<MessageEnvelope<U>>>,
                        envelope: MessageEnvelope<U>| {
      match envelope {
        MessageEnvelope::User(message) => {
          // Inline behavior.handle to avoid lifetime issues in closure
          let mut typed_ctx = TypedContext { inner: ctx };
          (behavior.handler)(&mut typed_ctx, message);
        }
        MessageEnvelope::System(message) => {
          if let Some(handler) = system_handler.as_mut() {
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

  pub async fn run_until<F>(
    &mut self,
    should_continue: F,
  ) -> Result<(), QueueError<PriorityEnvelope<MessageEnvelope<U>>>>
  where
    F: FnMut() -> bool, {
    self.inner.run_until(should_continue).await
  }

  pub async fn run_forever(&mut self) -> Result<Infallible, QueueError<PriorityEnvelope<MessageEnvelope<U>>>> {
    self.inner.run_forever().await
  }

  #[cfg(feature = "std")]
  pub fn blocking_dispatch_loop<F>(
    &mut self,
    should_continue: F,
  ) -> Result<(), QueueError<PriorityEnvelope<MessageEnvelope<U>>>>
  where
    F: FnMut() -> bool, {
    self.inner.blocking_dispatch_loop(should_continue)
  }

  #[cfg(feature = "std")]
  pub fn blocking_dispatch_forever(&mut self) -> Result<Infallible, QueueError<PriorityEnvelope<MessageEnvelope<U>>>> {
    self.inner.blocking_dispatch_forever()
  }

  pub async fn dispatch_next(&mut self) -> Result<(), QueueError<PriorityEnvelope<MessageEnvelope<U>>>> {
    self.inner.dispatch_next().await
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

  #[deprecated(since = "3.1.0", note = "dispatch_next / run_until を使用してください")]
  pub fn dispatch_all(&mut self) -> Result<(), QueueError<PriorityEnvelope<MessageEnvelope<U>>>> {
    #[allow(deprecated)]
    self.inner.dispatch_all()
  }

  pub async fn dispatch_next(&mut self) -> Result<(), QueueError<PriorityEnvelope<MessageEnvelope<U>>>> {
    self.inner.dispatch_next().await
  }

  pub fn raw(&mut self) -> &mut RootContext<'a, MessageEnvelope<U>, R, Strat> {
    &mut self.inner
  }
}

#[cfg(test)]
mod tests {
  #![allow(deprecated)]
  use super::*;
  use crate::mailbox::test_support::TestMailboxRuntime;
  use alloc::rc::Rc;
  use alloc::vec::Vec;
  use core::cell::RefCell;
  use futures::executor::block_on;

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
    block_on(root.dispatch_next()).expect("dispatch");

    assert_eq!(log.borrow().as_slice(), &[11]);
  }
}
