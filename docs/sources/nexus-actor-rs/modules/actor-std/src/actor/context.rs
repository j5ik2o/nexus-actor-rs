use std::fmt::Debug;
use std::time::Duration;

use async_trait::async_trait;

use crate::actor::actor_system::ActorSystem;
use crate::actor::core::ActorError;
use crate::actor::core::ActorHandle;
use crate::actor::core::Continuer;
use crate::actor::core::ExtendedPid;
use crate::actor::core::Props;
use crate::actor::core::SpawnError;
use crate::actor::message::MessageEnvelope;
use crate::actor::message::MessageHandle;
use crate::actor::message::ReadonlyMessageHeadersHandle;
use crate::actor::message::ResponseHandle;
use crate::ctxext::extensions::{ContextExtensionHandle, ContextExtensionId};
use nexus_actor_core_rs::CorePid;

mod actor_context;
mod actor_context_extras;
mod base_spawner;
mod context_handle;
mod context_registry;
mod context_snapshot;
mod core_context_adapter;
mod lock_timing;
mod mock_context;
mod receive_timeout_timer;
mod receiver_context_handle;
mod receiver_snapshot;
mod root_context;
mod sender_context_handle;
mod spawner_context_handle;
mod state;
mod typed_actor_context;
mod typed_context_borrow;
mod typed_context_handle;
mod typed_context_snapshot;
mod typed_root_context;

use crate::actor::process::actor_future::ActorFuture;
pub use {
  self::actor_context::*, self::base_spawner::*, self::context_handle::*, self::context_registry::ContextRegistry,
  self::context_snapshot::*, self::core_context_adapter::StdActorContextSnapshot,
  self::lock_timing::lock_wait_snapshot, self::lock_timing::LockStatRecord, self::mock_context::*,
  self::receiver_context_handle::*, self::receiver_snapshot::*, self::root_context::*, self::sender_context_handle::*,
  self::spawner_context_handle::*, self::typed_context_borrow::*, self::typed_context_handle::*,
  self::typed_context_snapshot::*, self::typed_root_context::*,
};

pub trait Context:
  ExtensionContext
  + SenderContext
  + ReceiverContext
  + SpawnerContext
  + BasePart
  + StopperPart
  + Debug
  + Send
  + Sync
  + 'static {
}

pub trait ExtensionContext: ExtensionPart + Send + Sync + 'static {}

pub trait SenderContext: InfoPart + SenderPart + CoreSenderPart + MessagePart + Send + Sync + 'static {}

pub trait ReceiverContext: InfoPart + ReceiverPart + MessagePart + ExtensionPart + Send + Sync + 'static {}

pub trait SpawnerContext: InfoPart + SpawnerPart + Send + Sync + 'static {}

#[async_trait]
pub trait ExtensionPart: Send + Sync + 'static {
  async fn get(&mut self, id: ContextExtensionId) -> Option<ContextExtensionHandle>;
  async fn set(&mut self, ext: ContextExtensionHandle);
}

#[async_trait]
pub trait InfoPart: Debug + Send + Sync + 'static {
  // Parent returns the PID for the current actors parent
  async fn get_parent(&self) -> Option<ExtendedPid>;

  // Self returns the PID for the current actor
  async fn get_self_opt(&self) -> Option<ExtendedPid>;
  async fn get_self(&self) -> ExtendedPid {
    self.get_self_opt().await.expect("self pid not found")
  }

  async fn set_self(&mut self, pid: ExtendedPid);

  // Actor returns the actor associated with this context
  async fn get_actor(&self) -> Option<ActorHandle>;

  async fn get_actor_system(&self) -> ActorSystem;
}

#[async_trait]
pub trait BasePart: Debug + Send + Sync + 'static {
  fn as_any(&self) -> &dyn std::any::Any;
  // ReceiveTimeout returns the current timeout
  async fn get_receive_timeout(&self) -> Duration;

  // Children returns a slice of the actors children
  async fn get_children(&self) -> Vec<ExtendedPid>;

  // Respond sends a response to the current `Sender`
  // If the Sender is nil, the actor will panic
  async fn respond(&self, response: ResponseHandle);

  // Stash stashes the current message on a stack for reprocessing when the actor restarts
  async fn stash(&mut self);
  async fn un_stash_all(&mut self) -> Result<(), ActorError>;

  // Watch registers the actor as a monitor for the specified PID
  async fn watch(&mut self, pid: &ExtendedPid);
  async fn watch_core(&mut self, pid: &CorePid) {
    self.watch(&ExtendedPid::from(pid.clone())).await;
  }

  // Unwatch unregisters the actor as a monitor for the specified PID
  async fn unwatch(&mut self, pid: &ExtendedPid);
  async fn unwatch_core(&mut self, pid: &CorePid) {
    self.unwatch(&ExtendedPid::from(pid.clone())).await;
  }

  // SetReceiveTimeout sets the inactivity timeout, after which a ReceiveTimeout message will be sent to the actor.
  // A duration of less than 1ms will disable the inactivity timer.
  //
  // If a message is received before the duration d, the timer will be reset. If the message conforms to
  // the not_influence_receive_timeout interface, the timer will not be reset
  async fn set_receive_timeout(&mut self, d: &Duration);

  async fn cancel_receive_timeout(&mut self);

  // Forward forwards current message to the given PID
  async fn forward(&self, pid: &ExtendedPid);
  async fn forward_core(&self, pid: &CorePid) {
    self.forward(&ExtendedPid::from(pid.clone())).await;
  }

  async fn reenter_after(&self, f: ActorFuture, continuation: Continuer);
}

#[async_trait]
pub trait MessagePart: Debug + Send + Sync + 'static {
  async fn get_message_envelope_opt(&self) -> Option<MessageEnvelope>;

  #[deprecated(
    since = "1.1.0",
    note = "Use get_message_envelope_opt().await or try_message_envelope()"
  )]
  async fn get_message_envelope(&self) -> MessageEnvelope {
    self
      .get_message_envelope_opt()
      .await
      .expect("message envelope not found")
  }

  // Message returns the current message to be processed
  async fn get_message_handle_opt(&self) -> Option<MessageHandle>;

  #[deprecated(since = "1.1.0", note = "Use get_message_handle_opt().await or try_message_handle()")]
  async fn get_message_handle(&self) -> MessageHandle {
    self.get_message_handle_opt().await.expect("message not found")
  }

  // MessageHeader returns the meta information for the currently processed message
  async fn get_message_header_handle(&self) -> Option<ReadonlyMessageHeadersHandle>;

  #[deprecated(
    since = "1.1.0",
    note = "Use get_message_header_handle().await or try_message_header()"
  )]
  async fn get_message_header(&self) -> ReadonlyMessageHeadersHandle {
    self
      .get_message_header_handle()
      .await
      .expect("message header not found")
  }
}

#[async_trait]
pub trait SenderPart: Debug + Send + Sync + 'static {
  // Sender returns the PID of actor that sent currently processed message
  async fn get_sender(&self) -> Option<ExtendedPid>;

  // Send sends a message to the given PID
  async fn send(&mut self, pid: ExtendedPid, message_handle: MessageHandle);

  // Request sends a message to the given PID
  async fn request(&mut self, pid: ExtendedPid, message_handle: MessageHandle);

  // RequestWithCustomSender sends a message to the given PID and also provides a Sender PID
  async fn request_with_custom_sender(&mut self, pid: ExtendedPid, message_handle: MessageHandle, sender: ExtendedPid);

  // RequestFuture sends a message to a given PID and returns a Future
  async fn request_future(&self, pid: ExtendedPid, message_handle: MessageHandle, timeout: Duration) -> ActorFuture;
}

#[async_trait]
pub trait CoreSenderPart: Debug + Send + Sync + 'static {
  async fn get_sender_core(&self) -> Option<CorePid>;

  async fn send_core(&mut self, pid: CorePid, message_handle: MessageHandle);

  async fn request_core(&mut self, pid: CorePid, message_handle: MessageHandle);

  async fn request_with_custom_sender_core(&mut self, pid: CorePid, message_handle: MessageHandle, sender: CorePid);

  async fn request_future_core(&self, pid: CorePid, message_handle: MessageHandle, timeout: Duration) -> ActorFuture;
}

#[async_trait]
pub trait ReceiverPart: Debug + Send + Sync + 'static {
  async fn receive(&mut self, envelope: MessageEnvelope) -> Result<(), ActorError>;
}

#[async_trait]
pub trait SpawnerPart: Send + Sync + 'static {
  // Spawn starts a new child actor based on props and named with a unique id
  async fn spawn(&mut self, props: Props) -> ExtendedPid;

  // SpawnPrefix starts a new child actor based on props and named using a prefix followed by a unique id
  async fn spawn_prefix(&mut self, props: Props, prefix: &str) -> ExtendedPid;

  // SpawnNamed starts a new child actor based on props and named using the specified name
  //
  // ErrNameExists will be returned if id already exists
  //
  // Please do not use name sharing same pattern with system actors, for example "YourPrefix$1", "Remote$1", "future$1"
  async fn spawn_named(&mut self, props: Props, id: &str) -> Result<ExtendedPid, SpawnError>;
}

#[async_trait]
pub trait StopperPart: Debug + Send + Sync + 'static {
  // Stop will stop actor immediately regardless of existing user messages in mailbox.
  async fn stop(&mut self, pid: &ExtendedPid);

  // StopFuture will stop actor immediately regardless of existing user messages in mailbox, and return its future.
  async fn stop_future_with_timeout(&mut self, pid: &ExtendedPid, timeout: Duration) -> ActorFuture;

  async fn stop_future(&mut self, pid: &ExtendedPid) -> ActorFuture {
    self.stop_future_with_timeout(pid, Duration::from_secs(10)).await
  }

  // Poison will tell actor to stop after processing current user messages in mailbox.
  async fn poison(&mut self, pid: &ExtendedPid);

  // PoisonFuture will tell actor to stop after processing current user messages in mailbox, and return its future.
  async fn poison_future_with_timeout(&mut self, pid: &ExtendedPid, timeout: Duration) -> ActorFuture;

  async fn poison_future(&mut self, pid: &ExtendedPid) -> ActorFuture {
    self.stop_future_with_timeout(pid, Duration::from_secs(10)).await
  }
}
