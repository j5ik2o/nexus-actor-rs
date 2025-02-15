//! Actor context module.

use std::any::Any;
use std::fmt::Debug;
use std::time::Duration;

use crate::actor::message::{MessageHandle, MessageOrEnvelope, ReadonlyMessageHeadersHandle, ResponseHandle};
use crate::actor::pid::Pid;
use crate::actor::{ActorError, ActorHandle, ActorSystem, Props, SpawnError};

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

// Core traits
pub trait Context: Send + Sync + 'static {
  fn as_any(&self) -> &dyn Any;
}

#[async_trait::async_trait]
pub trait InfoPart: Context {
  async fn get_children(&self) -> Vec<Pid>;
  async fn get_receive_timeout(&self) -> Duration;
  async fn get_parent(&self) -> Option<Pid>;
  async fn get_self_opt(&self) -> Option<Pid>;
  async fn set_self(&mut self, pid: Pid);
  async fn get_actor(&self) -> Option<ActorHandle>;
  async fn get_actor_system(&self) -> ActorSystem;
}

#[async_trait::async_trait]
pub trait MessagePart: Context {
  async fn get_message(&self) -> MessageHandle;
  async fn get_message_header_handle(&self) -> Option<ReadonlyMessageHeadersHandle>;
  async fn get_message_envelope_opt(&self) -> Option<MessageOrEnvelope>;
  async fn get_message_handle_opt(&self) -> Option<MessageHandle>;
}

#[async_trait::async_trait]
pub trait ReceiverPart: Context {
  async fn receive(&self, envelope: MessageOrEnvelope) -> Result<(), ActorError>;
}

#[async_trait::async_trait]
pub trait SenderPart: Context {
  async fn forward(&self, pid: &Pid);
  async fn respond(&self, response: ResponseHandle);
  async fn get_sender(&self) -> Option<Pid>;
  async fn send(&mut self, pid: Pid, message_handle: MessageHandle);
  async fn request(&mut self, pid: Pid, message_handle: MessageHandle);
  async fn request_with_custom_sender(&mut self, pid: Pid, message_handle: MessageHandle, sender: Pid);
  async fn request_future(&self, pid: Pid, message_handle: MessageHandle) -> Result<ResponseHandle, ActorError>;
}

#[async_trait::async_trait]
pub trait SpawnerPart: Context {
  async fn spawn(&self, props: Props) -> Result<Pid, SpawnError>;
  async fn spawn_prefix(&self, props: Props, prefix: &str) -> Result<Pid, SpawnError>;
  async fn spawn_named(&mut self, props: Props, id: &str) -> Result<Pid, SpawnError>;
}

#[async_trait::async_trait]
pub trait StopperPart: Context {
  async fn stop(&self);
  async fn poison_pill(&self);
  async fn stop_future_with_timeout(&mut self, pid: &Pid, timeout: Duration) -> Result<(), ActorError>;
  async fn poison(&mut self, pid: &Pid);
  async fn poison_future_with_timeout(&mut self, pid: &Pid, timeout: Duration) -> Result<(), ActorError>;
}

#[async_trait::async_trait]
pub trait ExtensionPart: Context {
  async fn register_extension<T: 'static>(&mut self, extension: T);
  async fn get_extension<T: 'static>(&self) -> Option<&T>;
  async fn get_extension_mut<T: 'static>(&mut self) -> Option<&mut T>;
}

pub trait ReceiverContext: ReceiverPart + MessagePart + InfoPart {}
pub trait SenderContext: SenderPart + MessagePart + InfoPart {}
pub trait SpawnerContext: SpawnerPart + MessagePart + InfoPart {}
pub trait ExtensionContext: ExtensionPart + MessagePart + InfoPart {}
