use alloc::boxed::Box;
use alloc::sync::Arc;

use crate::api::messaging::MessageEnvelope;
use crate::api::supervision::{NoopSupervisor, Supervisor, SupervisorDirective};
use crate::runtime::context::MapSystemFn;
use crate::runtime::message::DynMessage;
use crate::MailboxFactory;
use crate::PriorityEnvelope;
use crate::SystemMessage;
use nexus_utils_core_rs::Element;

use super::Context;

type ReceiveFn<U, R> = dyn for<'r, 'ctx> FnMut(&mut Context<'r, 'ctx, U, R>, U) -> BehaviorDirective<U, R> + 'static;
type SystemHandlerFn<U, R> = dyn for<'r, 'ctx> FnMut(&mut Context<'r, 'ctx, U, R>, SystemMessage) + 'static;
type SetupFn<U, R> = dyn for<'r, 'ctx> Fn(&mut Context<'r, 'ctx, U, R>) -> Behavior<U, R> + 'static;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SupervisorStrategyConfig {
  Default,
  Fixed(SupervisorStrategy),
}

impl SupervisorStrategyConfig {
  pub(crate) fn default() -> Self {
    SupervisorStrategyConfig::Default
  }

  pub(crate) fn from_strategy(strategy: SupervisorStrategy) -> Self {
    SupervisorStrategyConfig::Fixed(strategy)
  }

  pub(crate) fn into_supervisor<M>(&self) -> DynSupervisor<M>
  where
    M: Element, {
    let inner: Box<dyn Supervisor<M>> = match self {
      SupervisorStrategyConfig::Default => Box::new(NoopSupervisor),
      SupervisorStrategyConfig::Fixed(strategy) => Box::new(FixedDirectiveSupervisor::new(*strategy)),
    };
    DynSupervisor::new(inner)
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupervisorStrategy {
  Restart,
  Stop,
  Resume,
  Escalate,
}

impl From<SupervisorStrategy> for SupervisorDirective {
  fn from(value: SupervisorStrategy) -> Self {
    match value {
      SupervisorStrategy::Restart => SupervisorDirective::Restart,
      SupervisorStrategy::Stop => SupervisorDirective::Stop,
      SupervisorStrategy::Resume => SupervisorDirective::Resume,
      SupervisorStrategy::Escalate => SupervisorDirective::Escalate,
    }
  }
}

struct FixedDirectiveSupervisor {
  directive: SupervisorDirective,
}

impl FixedDirectiveSupervisor {
  fn new(strategy: SupervisorStrategy) -> Self {
    Self {
      directive: strategy.into(),
    }
  }
}

impl<M> Supervisor<M> for FixedDirectiveSupervisor {
  fn decide(&mut self, _error: &dyn core::fmt::Debug) -> SupervisorDirective {
    self.directive
  }
}

pub(crate) struct DynSupervisor<M>
where
  M: Element, {
  inner: Box<dyn Supervisor<M>>,
}

impl<M> DynSupervisor<M>
where
  M: Element,
{
  fn new(inner: Box<dyn Supervisor<M>>) -> Self {
    Self { inner }
  }
}

impl<M> Supervisor<M> for DynSupervisor<M>
where
  M: Element,
{
  fn before_handle(&mut self) {
    self.inner.before_handle();
  }

  fn after_handle(&mut self) {
    self.inner.after_handle();
  }

  fn decide(&mut self, error: &dyn core::fmt::Debug) -> SupervisorDirective {
    self.inner.decide(error)
  }
}

/// ユーザーメッセージ処理後の状態遷移指示。
pub enum BehaviorDirective<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone, {
  Same,
  Become(Behavior<U, R>),
}

pub struct BehaviorState<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone, {
  handler: Box<ReceiveFn<U, R>>,
  supervisor: SupervisorStrategyConfig,
}

impl<U, R> BehaviorState<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
{
  fn new(handler: Box<ReceiveFn<U, R>>, supervisor: SupervisorStrategyConfig) -> Self {
    Self { handler, supervisor }
  }
}

/// Typed Behavior 表現。Akka/Pekko Typed の `Behavior` に相当する。
pub enum Behavior<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone, {
  Receive(BehaviorState<U, R>),
  Setup(Arc<SetupFn<U, R>>),
  Stopped,
}

impl<U, R> Behavior<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
{
  pub fn receive<F>(handler: F) -> Self
  where
    F: for<'r, 'ctx> FnMut(&mut Context<'r, 'ctx, U, R>, U) -> BehaviorDirective<U, R> + 'static, {
    Self::Receive(BehaviorState::new(
      Box::new(handler),
      SupervisorStrategyConfig::default(),
    ))
  }

  pub fn stateless<F>(mut handler: F) -> Self
  where
    F: for<'r, 'ctx> FnMut(&mut Context<'r, 'ctx, U, R>, U) + 'static, {
    Self::Receive(BehaviorState::new(
      Box::new(move |ctx, msg| {
        handler(ctx, msg);
        BehaviorDirective::Same
      }),
      SupervisorStrategyConfig::default(),
    ))
  }

  pub fn stopped() -> Self {
    Self::Stopped
  }

  pub fn setup<F>(init: F) -> Self
  where
    F: for<'r, 'ctx> Fn(&mut Context<'r, 'ctx, U, R>) -> Behavior<U, R> + 'static, {
    Self::Setup(Arc::new(init))
  }

  pub(crate) fn supervisor_config(&self) -> SupervisorStrategyConfig {
    match self {
      Behavior::Receive(state) => state.supervisor.clone(),
      Behavior::Setup(_) | Behavior::Stopped => SupervisorStrategyConfig::default(),
    }
  }
}

/// Behavior DSL ビルダー。
pub struct Behaviors;

impl Behaviors {
  pub fn receive<U, R, F>(handler: F) -> Behavior<U, R>
  where
    U: Element,
    R: MailboxFactory + Clone + 'static,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone,
    R::Signal: Clone,
    F: for<'r, 'ctx> FnMut(&mut Context<'r, 'ctx, U, R>, U) -> BehaviorDirective<U, R> + 'static, {
    Behavior::receive(handler)
  }

  pub fn same<U, R>() -> BehaviorDirective<U, R>
  where
    U: Element,
    R: MailboxFactory + Clone + 'static,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone,
    R::Signal: Clone, {
    BehaviorDirective::Same
  }

  pub fn transition<U, R>(behavior: Behavior<U, R>) -> BehaviorDirective<U, R>
  where
    U: Element,
    R: MailboxFactory + Clone + 'static,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone,
    R::Signal: Clone, {
    BehaviorDirective::Become(behavior)
  }

  pub fn stopped<U, R>() -> Behavior<U, R>
  where
    U: Element,
    R: MailboxFactory + Clone + 'static,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone,
    R::Signal: Clone, {
    Behavior::stopped()
  }

  pub fn supervise<U, R>(behavior: Behavior<U, R>) -> SuperviseBuilder<U, R>
  where
    U: Element,
    R: MailboxFactory + Clone + 'static,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone,
    R::Signal: Clone, {
    SuperviseBuilder { behavior }
  }

  pub fn setup<U, R, F>(init: F) -> Behavior<U, R>
  where
    U: Element,
    R: MailboxFactory + Clone + 'static,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone,
    R::Signal: Clone,
    F: for<'r, 'ctx> Fn(&mut Context<'r, 'ctx, U, R>) -> Behavior<U, R> + 'static, {
    Behavior::setup(init)
  }
}

pub struct SuperviseBuilder<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone, {
  behavior: Behavior<U, R>,
}

impl<U, R> SuperviseBuilder<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
{
  pub fn with_strategy(mut self, strategy: SupervisorStrategy) -> Behavior<U, R> {
    if let Behavior::Receive(state) = &mut self.behavior {
      state.supervisor = SupervisorStrategyConfig::from_strategy(strategy);
    }
    self.behavior
  }
}

/// Behavior を非Typedランタイムへ橋渡しするアダプタ。
pub struct ActorAdapter<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone, {
  behavior_factory: Arc<dyn Fn() -> Behavior<U, R> + 'static>,
  pub(super) behavior: Behavior<U, R>,
  pub(super) system_handler: Option<Box<SystemHandlerFn<U, R>>>,
}

impl<U, R> ActorAdapter<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
{
  pub fn new<S>(behavior_factory: Arc<dyn Fn() -> Behavior<U, R> + 'static>, system_handler: Option<S>) -> Self
  where
    S: for<'r, 'ctx> FnMut(&mut Context<'r, 'ctx, U, R>, SystemMessage) + 'static, {
    let behavior = behavior_factory();
    Self {
      behavior_factory,
      behavior,
      system_handler: system_handler.map(|h| Box::new(h) as Box<SystemHandlerFn<U, R>>),
    }
  }

  pub fn handle_user(&mut self, ctx: &mut Context<'_, '_, U, R>, message: U) {
    self.ensure_initialized(ctx);
    match &mut self.behavior {
      Behavior::Receive(state) => {
        let handler = state.handler.as_mut();
        match handler(ctx, message) {
          BehaviorDirective::Same => {}
          BehaviorDirective::Become(next) => {
            self.behavior = next;
          }
        }
      }
      Behavior::Stopped => {
        // ignore further user messages
      }
      Behavior::Setup(_) => unreachable!(),
    }
  }

  pub fn handle_system(&mut self, ctx: &mut Context<'_, '_, U, R>, message: SystemMessage) {
    self.ensure_initialized(ctx);
    if matches!(message, SystemMessage::Stop) {
      self.behavior = Behavior::stopped();
    } else if matches!(message, SystemMessage::Restart) {
      self.behavior = (self.behavior_factory)();
      self.ensure_initialized(ctx);
    }
    if let Some(handler) = self.system_handler.as_mut() {
      handler(ctx, message);
    }
  }

  /// Guardian/Scheduler 用の SystemMessage マッパー。
  pub fn create_map_system() -> Arc<MapSystemFn<DynMessage>> {
    Arc::new(|sys| DynMessage::new(MessageEnvelope::<U>::System(sys)))
  }

  pub(crate) fn supervisor_config(&self) -> SupervisorStrategyConfig {
    self.behavior.supervisor_config()
  }

  fn ensure_initialized(&mut self, ctx: &mut Context<'_, '_, U, R>) {
    loop {
      match &self.behavior {
        Behavior::Setup(init) => {
          let init = init.clone();
          self.behavior = init(ctx);
        }
        _ => break,
      }
    }
  }
}
