use crate::actor::actor_system::ActorSystem;
use crate::actor::context::{BasePart, ExtensionContext, ExtensionPart};
use crate::actor::core::{ActorError, ActorHandle, SpawnError, TypedExtendedPid, TypedProps};
use crate::actor::message::{Message, MessageHandle, ReadonlyMessageHeadersHandle, TypedMessageEnvelope};
use crate::actor::process::actor_future::ActorFuture;
use async_trait::async_trait;
use std::fmt::Debug;
use std::time::Duration;

pub trait TypedContext<M: Message>:
  ExtensionContext
  + TypedSenderContext<M>
  + TypedReceiverContext<M>
  + TypedSpawnerContext<M>
  + BasePart
  + TypedStopperPart<M>
  + Debug
  + Send
  + Sync
  + 'static {
}

pub trait TypedSenderContext<M: Message>:
  TypedInfoPart<M> + TypedSenderPart<M> + TypedMessagePart<M> + Send + Sync + 'static {
}

pub trait TypedReceiverContext<M: Message>:
  TypedInfoPart<M> + TypedReceiverPart<M> + TypedMessagePart<M> + ExtensionPart + Send + Sync + 'static {
}
pub trait TypedSpawnerContext<M: Message>: TypedInfoPart<M> + TypedSpawnerPart + Send + Sync + 'static {}

/// 同期スナップショット経由で Typed コンテキストの状態にアクセスするための補助トレイト。
/// 既存の非同期 getter から移行する際の代替 API として利用する。
pub trait TypedContextSyncView<M: Message>: Send + Sync {
  /// `ActorSystem` をスナップショットから取得できる場合は返す。
  fn actor_system_snapshot(&self) -> Option<ActorSystem> {
    None
  }

  /// `ActorHandle` をスナップショットから取得できる場合は返す。
  fn actor_snapshot(&self) -> Option<ActorHandle> {
    None
  }

  /// 親 PID の同期スナップショット。
  fn parent_snapshot(&self) -> Option<TypedExtendedPid<M>> {
    None
  }

  /// 自身の PID の同期スナップショット。
  fn self_snapshot(&self) -> Option<TypedExtendedPid<M>> {
    None
  }

  /// メッセージハンドルの同期スナップショット。
  fn message_handle_snapshot(&self) -> Option<MessageHandle> {
    None
  }

  /// メッセージの同期スナップショット。
  fn message_snapshot(&self) -> Option<M>
  where
    M: Clone, {
    None
  }

  /// メッセージヘッダの同期スナップショット。
  fn message_header_snapshot(&self) -> Option<ReadonlyMessageHeadersHandle> {
    None
  }

  /// 送信者 PID の同期スナップショット。
  fn sender_snapshot(&self) -> Option<TypedExtendedPid<M>> {
    None
  }
}

#[async_trait]
pub trait TypedInfoPart<M: Message>: Debug + Send + Sync + 'static {
  // Parent returns the PID for the current actors parent
  async fn get_parent(&self) -> Option<TypedExtendedPid<M>>;

  // Self returns the PID for the current actor
  async fn get_self_opt(&self) -> Option<TypedExtendedPid<M>>;
  async fn get_self(&self) -> TypedExtendedPid<M> {
    self.get_self_opt().await.expect("self pid not found")
  }

  async fn set_self(&mut self, pid: TypedExtendedPid<M>);

  // Actor returns the actor associated with this context
  async fn get_actor(&self) -> Option<ActorHandle>;

  async fn get_actor_system(&self) -> ActorSystem;
}
#[async_trait]
pub trait TypedMessagePart<M: Message>: Debug + Send + Sync + 'static {
  async fn get_message_envelope_opt(&self) -> Option<TypedMessageEnvelope<M>>;

  #[deprecated(
    since = "1.1.0",
    note = "Use get_message_envelope_opt().await or try_message_envelope()"
  )]
  async fn get_message_envelope(&self) -> TypedMessageEnvelope<M> {
    self
      .get_message_envelope_opt()
      .await
      .expect("message envelope not found")
  }

  async fn get_message_handle_opt(&self) -> Option<MessageHandle>;

  #[deprecated(since = "1.1.0", note = "Use get_message_handle_opt().await or try_message_opt()")]
  async fn get_message_handle(&self) -> MessageHandle {
    self.get_message_handle_opt().await.expect("message handle not found")
  }

  // Message returns the current message to be processed
  async fn get_message_opt(&self) -> Option<M>;

  async fn get_message(&self) -> M {
    self.get_message_opt().await.expect("message not found")
  }

  // MessageHeader returns the meta information for the currently processed message
  async fn get_message_header_handle(&self) -> Option<ReadonlyMessageHeadersHandle>;
}
#[async_trait]
pub trait TypedSenderPart<M: Message>: Debug + Send + Sync + 'static {
  // Sender returns the PID of actor that sent currently processed message
  async fn get_sender(&self) -> Option<TypedExtendedPid<M>>;

  // Send sends a message to the given PID
  async fn send<A: Message>(&mut self, pid: TypedExtendedPid<A>, message: A);

  // Request sends a message to the given PID
  async fn request<A: Message>(&mut self, pid: TypedExtendedPid<A>, message: A);

  // RequestWithCustomSender sends a message to the given PID and also provides a Sender PID
  async fn request_with_custom_sender<A: Message, B: Message>(
    &mut self,
    pid: TypedExtendedPid<A>,
    message: A,
    sender: TypedExtendedPid<B>,
  );

  // RequestFuture sends a message to a given PID and returns a Future
  async fn request_future<A: Message>(&self, pid: TypedExtendedPid<A>, message: A, timeout: Duration) -> ActorFuture;
}
#[async_trait]
pub trait TypedReceiverPart<M: Message>: Debug + Send + Sync + 'static {
  async fn receive(&mut self, envelope: TypedMessageEnvelope<M>) -> Result<(), ActorError>;
}

#[async_trait]
pub trait TypedSpawnerPart: Send + Sync + 'static {
  // Spawn starts a new child actor based on props and named with a unique id
  async fn spawn<A: Message + Clone>(&mut self, props: TypedProps<A>) -> TypedExtendedPid<A>;

  // SpawnPrefix starts a new child actor based on props and named using a prefix followed by a unique id
  async fn spawn_prefix<A: Message + Clone>(&mut self, props: TypedProps<A>, prefix: &str) -> TypedExtendedPid<A>;

  // SpawnNamed starts a new child actor based on props and named using the specified name
  //
  // ErrNameExists will be returned if id already exists
  //
  // Please do not use name sharing same pattern with system actors, for example "YourPrefix$1", "Remote$1", "future$1"
  async fn spawn_named<A: Message + Clone>(
    &mut self,
    props: TypedProps<A>,
    id: &str,
  ) -> Result<TypedExtendedPid<A>, SpawnError>;
}

#[async_trait]
pub trait TypedStopperPart<M: Message>: Debug + Send + Sync + 'static {
  // Stop will stop actor immediately regardless of existing user messages in mailbox.
  async fn stop(&mut self, pid: &TypedExtendedPid<M>);

  // StopFuture will stop actor immediately regardless of existing user messages in mailbox, and return its future.
  async fn stop_future_with_timeout(&mut self, pid: &TypedExtendedPid<M>, timeout: Duration) -> ActorFuture;

  async fn stop_future(&mut self, pid: &TypedExtendedPid<M>) -> ActorFuture {
    self.stop_future_with_timeout(pid, Duration::from_secs(10)).await
  }

  // Poison will tell actor to stop after processing current user messages in mailbox.
  async fn poison(&mut self, pid: &TypedExtendedPid<M>);

  // PoisonFuture will tell actor to stop after processing current user messages in mailbox, and return its future.
  async fn poison_future_with_timeout(&mut self, pid: &TypedExtendedPid<M>, timeout: Duration) -> ActorFuture;

  async fn poison_future(&mut self, pid: &TypedExtendedPid<M>) -> ActorFuture {
    self.stop_future_with_timeout(pid, Duration::from_secs(10)).await
  }
}
