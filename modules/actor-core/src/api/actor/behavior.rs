use alloc::boxed::Box;
use alloc::sync::Arc;

use crate::api::messaging::MessageEnvelope;
use crate::runtime::context::MapSystemFn;
use crate::runtime::message::DynMessage;
use crate::MailboxFactory;
use crate::PriorityEnvelope;
use crate::SystemMessage;
use nexus_utils_core_rs::Element;

use super::Context;

type ReceiveFn<U, R> = dyn for<'r, 'ctx> FnMut(&mut Context<'r, 'ctx, U, R>, U) -> BehaviorDirective<U, R> + 'static;
type SystemHandlerFn<U, R> = dyn for<'r, 'ctx> FnMut(&mut Context<'r, 'ctx, U, R>, SystemMessage) + 'static;

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

/// Typed Behavior 表現。Akka/Pekko Typed の `Behavior` に相当する。
pub enum Behavior<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone, {
  Receive(Box<ReceiveFn<U, R>>),
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
    Self::Receive(Box::new(handler))
  }

  pub fn stateless<F>(mut handler: F) -> Self
  where
    F: for<'r, 'ctx> FnMut(&mut Context<'r, 'ctx, U, R>, U) + 'static, {
    Self::Receive(Box::new(move |ctx, msg| {
      handler(ctx, msg);
      BehaviorDirective::Same
    }))
  }

  pub fn stopped() -> Self {
    Self::Stopped
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
}

/// Behavior を非Typedランタイムへ橋渡しするアダプタ。
pub struct ActorAdapter<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone, {
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
  pub fn new<S>(behavior: Behavior<U, R>, system_handler: Option<S>) -> Self
  where
    S: for<'r, 'ctx> FnMut(&mut Context<'r, 'ctx, U, R>, SystemMessage) + 'static, {
    Self {
      behavior,
      system_handler: system_handler.map(|h| Box::new(h) as Box<SystemHandlerFn<U, R>>),
    }
  }

  pub fn handle_user(&mut self, ctx: &mut Context<'_, '_, U, R>, message: U) {
    match &mut self.behavior {
      Behavior::Receive(handler) => match handler(ctx, message) {
        BehaviorDirective::Same => {}
        BehaviorDirective::Become(next) => {
          self.behavior = next;
        }
      },
      Behavior::Stopped => {
        // ignore further user messages
      }
    }
  }

  pub fn handle_system(&mut self, ctx: &mut Context<'_, '_, U, R>, message: SystemMessage) {
    if matches!(message, SystemMessage::Stop) {
      self.behavior = Behavior::stopped();
    }
    if let Some(handler) = self.system_handler.as_mut() {
      handler(ctx, message);
    }
  }

  /// Guardian/Scheduler 用の SystemMessage マッパー。
  pub fn create_map_system() -> Arc<MapSystemFn<DynMessage>> {
    Arc::new(|sys| DynMessage::new(MessageEnvelope::<U>::System(sys)))
  }
}
