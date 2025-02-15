//! Actor context module.

pub mod actor_context;
pub mod actor_context_extras;
pub mod actor_context_test;
pub mod context_handle;
pub mod mock_context;
pub mod receiver_context_handle;
pub mod root_context;
pub mod sender_context_handle;
pub mod spawner_context_handle;
pub mod typed_actor_context;
pub mod typed_context_handle;
pub mod typed_root_context;

pub use self::{
  actor_context::*, actor_context_extras::*, context_handle::*, mock_context::*, receiver_context_handle::*,
  root_context::*, sender_context_handle::*, spawner_context_handle::*, typed_actor_context::*,
  typed_context_handle::*, typed_root_context::*,
};

// Re-export common types
pub use crate::actor::{
  ActorHandle, ActorSystem, Continuer, ErrorReason, Message, MessageHandle, Pid, Process, ProcessHandle, Props,
  SenderMiddleware, SenderMiddlewareChain, SpawnError, Spawner,
};

// Re-export context traits
pub trait Context: Send + Sync + 'static {
  fn as_any(&self) -> &dyn std::any::Any;
}

pub trait InfoPart: Context {
  fn get_children(&self) -> Vec<Pid>;
  fn get_receive_timeout(&self) -> std::time::Duration;
}

pub trait MessagePart: Context {
  fn get_message(&self) -> MessageHandle;
  fn get_message_header_handle(&self) -> Option<ReadonlyMessageHeadersHandle>;
}

pub trait ReceiverPart: Context {
  fn receive(&self, envelope: MessageOrEnvelope) -> Result<(), ActorError>;
}

pub trait SenderPart: Context {
  fn forward(&self, pid: &Pid);
  fn respond(&self, response: ResponseHandle);
}

pub trait SpawnerPart: Context {
  fn spawn(&self, props: Props) -> Result<Pid, SpawnError>;
  fn spawn_prefix(&self, props: Props, prefix: &str) -> Result<Pid, SpawnError>;
}

pub trait StopperPart: Context {
  fn stop(&self);
  fn poison_pill(&self);
}

pub trait ExtensionPart: Context {
  fn register_extension<T: 'static>(&mut self, extension: T);
  fn get_extension<T: 'static>(&self) -> Option<&T>;
  fn get_extension_mut<T: 'static>(&mut self) -> Option<&mut T>;
}

pub trait ReceiverContext: ReceiverPart + MessagePart + InfoPart {}
pub trait SenderContext: SenderPart + MessagePart + InfoPart {}
pub trait SpawnerContext: SpawnerPart + MessagePart + InfoPart {}
pub trait ExtensionContext: ExtensionPart + MessagePart + InfoPart {}
