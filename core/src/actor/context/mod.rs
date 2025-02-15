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

// Core traits
pub trait Context: Send + Sync + 'static {
  fn as_any(&self) -> &dyn std::any::Any;
}

pub trait InfoPart: Context {
  fn get_children(&self) -> Vec<crate::actor::pid::Pid>;
  fn get_receive_timeout(&self) -> std::time::Duration;
  fn get_parent(&self) -> Option<crate::actor::pid::Pid>;
  fn get_self_opt(&self) -> Option<crate::actor::pid::Pid>;
  fn set_self(&mut self, pid: crate::actor::pid::Pid);
  fn get_actor(&self) -> Option<crate::actor::ActorHandle>;
  fn get_actor_system(&self) -> crate::actor::ActorSystem;
}

pub trait MessagePart: Context {
  fn get_message(&self) -> crate::actor::message::MessageHandle;
  fn get_message_header_handle(&self) -> Option<crate::actor::message::ReadonlyMessageHeadersHandle>;
  fn get_message_envelope_opt(&self) -> Option<crate::actor::message::MessageOrEnvelope>;
  fn get_message_handle_opt(&self) -> Option<crate::actor::message::MessageHandle>;
}

pub trait ReceiverPart: Context {
  fn receive(&self, envelope: crate::actor::message::MessageOrEnvelope) -> Result<(), crate::actor::ActorError>;
}

pub trait SenderPart: Context {
  fn forward(&self, pid: &crate::actor::pid::Pid);
  fn respond(&self, response: crate::actor::message::ResponseHandle);
  fn get_sender(&self) -> Option<crate::actor::pid::Pid>;
  fn send(&mut self, pid: crate::actor::pid::Pid, message_handle: crate::actor::message::MessageHandle);
  fn request(&mut self, pid: crate::actor::pid::Pid, message_handle: crate::actor::message::MessageHandle);
  fn request_with_custom_sender(
    &mut self,
    pid: crate::actor::pid::Pid,
    message_handle: crate::actor::message::MessageHandle,
    sender: crate::actor::pid::Pid,
  );
  fn request_future(
    &self,
    pid: crate::actor::pid::Pid,
    message_handle: crate::actor::message::MessageHandle,
  ) -> std::pin::Pin<
    Box<
      dyn std::future::Future<Output = Result<crate::actor::message::ResponseHandle, crate::actor::ActorError>> + Send,
    >,
  >;
}

pub trait SpawnerPart: Context {
  fn spawn(&self, props: crate::actor::Props) -> Result<crate::actor::pid::Pid, crate::actor::SpawnError>;
  fn spawn_prefix(
    &self,
    props: crate::actor::Props,
    prefix: &str,
  ) -> Result<crate::actor::pid::Pid, crate::actor::SpawnError>;
  fn spawn_named(
    &mut self,
    props: crate::actor::Props,
    id: &str,
  ) -> Result<crate::actor::pid::Pid, crate::actor::SpawnError>;
}

pub trait StopperPart: Context {
  fn stop(&self);
  fn poison_pill(&self);
  fn stop_future_with_timeout(
    &mut self,
    pid: &crate::actor::pid::Pid,
    timeout: std::time::Duration,
  ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), crate::actor::ActorError>> + Send>>;
  fn poison(&mut self, pid: &crate::actor::pid::Pid);
  fn poison_future_with_timeout(
    &mut self,
    pid: &crate::actor::pid::Pid,
    timeout: std::time::Duration,
  ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), crate::actor::ActorError>> + Send>>;
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
