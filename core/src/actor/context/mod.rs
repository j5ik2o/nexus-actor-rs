//! Actor context module.

use std::any::Any;
use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;
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

pub trait InfoPart: Context {
  fn get_children(&self) -> Vec<Pid>;
  fn get_receive_timeout(&self) -> Duration;
  fn get_parent(&self) -> Option<Pid>;
  fn get_self_opt(&self) -> Option<Pid>;
  fn set_self(&mut self, pid: Pid);
  fn get_actor(&self) -> Option<ActorHandle>;
  fn get_actor_system(&self) -> ActorSystem;
}

pub trait MessagePart: Context {
  fn get_message(&self) -> MessageHandle;
  fn get_message_header_handle(&self) -> Option<ReadonlyMessageHeadersHandle>;
  fn get_message_envelope_opt(&self) -> Option<MessageOrEnvelope>;
  fn get_message_handle_opt(&self) -> Option<MessageHandle>;
}

pub trait ReceiverPart: Context {
  fn receive(&self, envelope: MessageOrEnvelope) -> Result<(), ActorError>;
}

pub trait SenderPart: Context {
  fn forward(&self, pid: &Pid);
  fn respond(&self, response: ResponseHandle);
  fn get_sender(&self) -> Option<Pid>;
  fn send(&mut self, pid: Pid, message_handle: MessageHandle);
  fn request(&mut self, pid: Pid, message_handle: MessageHandle);
  fn request_with_custom_sender(&mut self, pid: Pid, message_handle: MessageHandle, sender: Pid);
  fn request_future(
    &self,
    pid: Pid,
    message_handle: MessageHandle,
  ) -> Pin<Box<dyn Future<Output = Result<ResponseHandle, ActorError>> + Send>>;
}

pub trait SpawnerPart: Context {
  fn spawn(&self, props: Props) -> Result<Pid, SpawnError>;
  fn spawn_prefix(&self, props: Props, prefix: &str) -> Result<Pid, SpawnError>;
  fn spawn_named(&mut self, props: Props, id: &str) -> Result<Pid, SpawnError>;
}

pub trait StopperPart: Context {
  fn stop(&self);
  fn poison_pill(&self);
  fn stop_future_with_timeout(
    &mut self,
    pid: &Pid,
    timeout: Duration,
  ) -> Pin<Box<dyn Future<Output = Result<(), ActorError>> + Send>>;
  fn poison(&mut self, pid: &Pid);
  fn poison_future_with_timeout(
    &mut self,
    pid: &Pid,
    timeout: Duration,
  ) -> Pin<Box<dyn Future<Output = Result<(), ActorError>> + Send>>;
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
