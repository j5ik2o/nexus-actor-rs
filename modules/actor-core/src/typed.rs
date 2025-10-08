use alloc::boxed::Box;
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

/// Adapter bridging Behavior and ActorContext, managing map_system generation.
pub struct TypedActorAdapter<U, R>
where
  U: Element,
  R: MailboxRuntime + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone, {
  behavior: Behavior<U, R>,
  system_handler: Option<
    Box<
      dyn for<'ctx> FnMut(
          &mut ActorContext<'ctx, MessageEnvelope<U>, R, dyn Supervisor<MessageEnvelope<U>>>,
          SystemMessage,
        ) + 'static,
    >,
  >,
}

impl<U, R> TypedActorAdapter<U, R>
where
  U: Element,
  R: MailboxRuntime + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone,
{
  pub fn new<S>(behavior: Behavior<U, R>, system_handler: Option<S>) -> Self
  where
    S: for<'ctx> FnMut(&mut ActorContext<'ctx, MessageEnvelope<U>, R, dyn Supervisor<MessageEnvelope<U>>>, SystemMessage)
      + 'static, {
    Self {
      behavior,
      system_handler: system_handler.map(|h| {
        Box::new(h)
          as Box<
            dyn for<'ctx> FnMut(
                &mut ActorContext<'ctx, MessageEnvelope<U>, R, dyn Supervisor<MessageEnvelope<U>>>,
                SystemMessage,
              ) + 'static,
          >
      }),
    }
  }

  pub fn handle_user(
    &mut self,
    ctx: &mut ActorContext<'_, MessageEnvelope<U>, R, dyn Supervisor<MessageEnvelope<U>>>,
    message: U,
  ) {
    let mut typed_ctx = TypedContext { inner: ctx };
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
  pub fn create_map_system() -> Arc<dyn Fn(SystemMessage) -> MessageEnvelope<U> + Send + Sync> {
    Arc::new(|sys: SystemMessage| MessageEnvelope::System(sys))
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
    behavior: Behavior<U, R>,
    system_handler: Option<S>,
  ) -> Self
  where
    S: for<'ctx> FnMut(&mut ActorContext<'ctx, MessageEnvelope<U>, R, dyn Supervisor<MessageEnvelope<U>>>, SystemMessage)
      + 'static, {
    let mut adapter = TypedActorAdapter::new(behavior, system_handler);
    let map_system = TypedActorAdapter::<U, R>::create_map_system();

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

  #[test]
  fn test_typed_actor_handles_system_stop() {
    let runtime = TestMailboxRuntime::unbounded();
    let mut system: TypedActorSystem<u32, _, AlwaysRestart> = TypedActorSystem::new(runtime);

    let stopped: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
    let stopped_clone = stopped.clone();

    let system_handler = move |_: &mut ActorContext<'_, MessageEnvelope<u32>, _, _>, sys_msg: SystemMessage| {
      if matches!(sys_msg, SystemMessage::Stop) {
        *stopped_clone.borrow_mut() = true;
      }
    };

    let props =
      TypedProps::with_system_handler(MailboxOptions::default(), move |_, _msg: u32| {}, Some(system_handler));

    let mut root = system.root_context();
    let actor_ref = root.spawn(props).expect("spawn typed actor");
    actor_ref.send_system(SystemMessage::Stop).expect("send stop");
    block_on(root.dispatch_next()).expect("dispatch");

    assert!(*stopped.borrow(), "SystemMessage::Stop should be handled");
  }

  #[test]
  fn test_typed_actor_handles_watch_unwatch() {
    let runtime = TestMailboxRuntime::unbounded();
    let mut system: TypedActorSystem<u32, _, AlwaysRestart> = TypedActorSystem::new(runtime);

    let watchers_count: Rc<RefCell<usize>> = Rc::new(RefCell::new(0));
    let watchers_count_clone = watchers_count.clone();

    let behavior = Behavior::stateless(move |ctx: &mut TypedContext<'_, '_, u32, _>, _msg: u32| {
      *watchers_count_clone.borrow_mut() = ctx.watchers().len();
    });

    let system_handler = Some(
      |ctx: &mut ActorContext<'_, _, _, _>, sys_msg: SystemMessage| match sys_msg {
        SystemMessage::Watch(watcher) => {
          ctx.register_watcher(watcher);
        }
        SystemMessage::Unwatch(watcher) => {
          ctx.unregister_watcher(watcher);
        }
        _ => {}
      },
    );

    let props = TypedProps::with_behavior_and_system(MailboxOptions::default(), behavior, system_handler);

    let mut root = system.root_context();
    let actor_ref = root.spawn(props).expect("spawn typed actor");

    // Get initial watcher count (parent is automatically registered)
    actor_ref.tell(1).expect("tell");
    block_on(root.dispatch_next()).expect("dispatch initial");
    let initial_count = *watchers_count.borrow();

    let watcher_id = ActorId(999);
    actor_ref
      .send_system(SystemMessage::Watch(watcher_id))
      .expect("send watch");
    block_on(root.dispatch_next()).expect("dispatch watch");

    actor_ref.tell(2).expect("tell");
    block_on(root.dispatch_next()).expect("dispatch user message");

    let after_watch_count = *watchers_count.borrow();
    assert_eq!(
      after_watch_count,
      initial_count + 1,
      "Watcher count should increase by 1"
    );

    actor_ref
      .send_system(SystemMessage::Unwatch(watcher_id))
      .expect("send unwatch");
    block_on(root.dispatch_next()).expect("dispatch unwatch");

    actor_ref.tell(3).expect("tell");
    block_on(root.dispatch_next()).expect("dispatch user message");

    let after_unwatch_count = *watchers_count.borrow();
    assert_eq!(
      after_unwatch_count, initial_count,
      "Watcher count should return to initial"
    );
  }

  #[test]
  fn test_typed_actor_stateful_behavior_with_system_message() {
    let runtime = TestMailboxRuntime::unbounded();
    let mut system: TypedActorSystem<u32, _, AlwaysRestart> = TypedActorSystem::new(runtime);

    // Stateful behavior: count user messages and track system messages
    let count = Rc::new(RefCell::new(0u32));
    let failures = Rc::new(RefCell::new(0u32));

    let count_clone = count.clone();
    let behavior = Behavior::stateless(move |_ctx: &mut TypedContext<'_, '_, u32, _>, msg: u32| {
      *count_clone.borrow_mut() += msg;
    });

    let failures_clone = failures.clone();
    let system_handler = move |_ctx: &mut ActorContext<'_, MessageEnvelope<u32>, _, _>, sys_msg: SystemMessage| {
      if matches!(sys_msg, SystemMessage::Suspend) {
        *failures_clone.borrow_mut() += 1;
      }
    };

    let props = TypedProps::with_behavior_and_system(MailboxOptions::default(), behavior, Some(system_handler));

    let mut root = system.root_context();
    let actor_ref = root.spawn(props).expect("spawn typed actor");

    // Send user messages
    actor_ref.tell(10).expect("tell 10");
    block_on(root.dispatch_next()).expect("dispatch user 1");

    actor_ref.tell(5).expect("tell 5");
    block_on(root.dispatch_next()).expect("dispatch user 2");

    // Send system message (Suspend doesn't stop the actor)
    actor_ref.send_system(SystemMessage::Suspend).expect("send suspend");
    block_on(root.dispatch_next()).expect("dispatch system");

    // Verify stateful behavior updated correctly
    assert_eq!(*count.borrow(), 15, "State should accumulate user messages");
    assert_eq!(*failures.borrow(), 1, "State should track system messages");
  }
}
