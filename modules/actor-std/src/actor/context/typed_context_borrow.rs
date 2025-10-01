use std::marker::PhantomData;

use crate::actor::actor_system::ActorSystem;
use crate::actor::context::actor_context::{ActorContext, ContextBorrow};
use crate::actor::context::{ContextHandle, ContextSnapshot, TypedContextSnapshot};
use crate::actor::core::{ActorHandle, ExtendedPid, Props, TypedExtendedPid};
use crate::actor::message::{Message, MessageEnvelope, MessageHandle, ReadonlyMessageHeadersHandle};

/// ライフタイム付きで `ActorContext` を参照する軽量ビュー。
/// 所有スナップショット (`TypedContextSnapshot`) を生成せずに props や sender を参照したい場面で利用する。
#[derive(Debug)]
pub struct TypedContextBorrow<'a, M: Message> {
  actor_context: &'a ActorContext,
  borrow: ContextBorrow<'a>,
  context_handle: ContextHandle,
  _marker: PhantomData<M>,
}

impl<'a, M: Message> TypedContextBorrow<'a, M> {
  pub(crate) fn new(actor_context: &'a ActorContext, context_handle: ContextHandle, borrow: ContextBorrow<'a>) -> Self {
    Self {
      actor_context,
      borrow,
      context_handle,
      _marker: PhantomData,
    }
  }

  /// 借用ビューから `TypedContextSnapshot` を生成する。
  pub fn snapshot(&self) -> TypedContextSnapshot<M> {
    let snapshot = ContextSnapshot::from_borrow(&self.borrow)
      .with_sender_opt(self.actor_context.try_sender())
      .with_message_envelope_opt(self.actor_context.try_message_envelope())
      .with_message_handle_opt(self.actor_context.try_message_handle())
      .with_message_header_opt(self.actor_context.try_message_header())
      .with_context_handle_opt(Some(self.context_handle.clone()));
    TypedContextSnapshot::new(snapshot)
  }

  /// 借用ビューを所有スナップショットへ変換する。
  pub fn into_snapshot(self) -> TypedContextSnapshot<M> {
    self.snapshot()
  }

  pub fn actor_system(&self) -> &ActorSystem {
    self.borrow.actor_system()
  }

  pub fn props(&self) -> &'a Props {
    self.borrow.props()
  }

  pub fn parent(&self) -> Option<&'a ExtendedPid> {
    self.borrow.parent()
  }

  pub fn self_pid(&self) -> Option<&ExtendedPid> {
    self.borrow.self_pid()
  }

  pub fn actor(&self) -> Option<&ActorHandle> {
    self.borrow.actor()
  }

  pub fn message_handle(&self) -> Option<MessageHandle> {
    self.actor_context.try_message_handle()
  }

  pub fn message_envelope(&self) -> Option<MessageEnvelope> {
    self.actor_context.try_message_envelope()
  }

  pub fn message_header(&self) -> Option<ReadonlyMessageHeadersHandle> {
    self.actor_context.try_message_header()
  }

  pub fn sender(&self) -> Option<TypedExtendedPid<M>> {
    self.actor_context.try_sender().map(|pid| pid.into())
  }

  pub fn context_handle(&self) -> &ContextHandle {
    &self.context_handle
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::actor::actor_system::ActorSystem;
  use crate::actor::core::{ActorError, Props};
  use crate::actor::message::Message;
  use crate::actor::typed_context::TypedContextSyncView;
  use std::any::Any;

  #[derive(Debug, Clone, PartialEq, Eq)]
  struct TestMessage;

  impl Message for TestMessage {
    fn eq_message(&self, other: &dyn Message) -> bool {
      other.as_any().downcast_ref::<Self>().is_some()
    }

    fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
      self
    }

    fn get_type_name(&self) -> String {
      "TestMessage".to_string()
    }
  }

  #[tokio::test]
  async fn borrow_exposes_message_without_snapshot() {
    let actor_system = ActorSystem::new().await.expect("actor system");
    let props = Props::from_async_actor_receiver(|_ctx| async { Ok::<(), ActorError>(()) }).await;
    let actor_context = ActorContext::new(actor_system.clone(), props, None).await;
    actor_context
      .inject_message_for_test(MessageHandle::new(TestMessage.clone()))
      .await;

    actor_context.with_typed_borrow::<TestMessage, _, _>(|view| {
      let handle = view.message_handle().expect("borrowed message");
      assert_eq!(handle.to_typed::<TestMessage>(), Some(TestMessage.clone()));
      let snapshot = view.snapshot();
      assert_eq!(snapshot.message_snapshot(), Some(TestMessage.clone()));
    });
  }
}
