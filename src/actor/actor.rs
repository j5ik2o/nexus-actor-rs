use std::any::Any;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::sync::Arc;

use async_trait::async_trait;
use backtrace::Backtrace;
use tokio::sync::Mutex;

use crate::actor::context::context_handle::ContextHandle;
use crate::actor::context::{InfoPart, MessagePart};
use crate::actor::message::auto_receive_message::AutoReceiveMessage;
use crate::actor::message::message_handle::{Message, MessageHandle};
use crate::actor::message::system_message::SystemMessage;
use crate::actor::supervisor::supervisor_strategy::SupervisorStrategyHandle;

pub mod actor_process;
pub mod actor_produce_func;
pub mod behavior;
pub mod context_decorator_chain_func;
pub mod context_decorator_func;
pub mod context_handler_func;
pub mod continuation_func;
pub mod pid;
pub mod pid_set;
#[cfg(test)]
mod pid_set_test;
pub mod props;
pub mod receive_func;
pub mod receiver_middleware_chain_func;
pub mod receiver_middleware_func;
pub mod restart_statistics;
pub mod sender_middleware_chain_func;
pub mod sender_middleware_func;
pub mod spawn_func;
pub mod spawn_middleware_func;
pub mod taks;

include!(concat!(env!("OUT_DIR"), "/actor.rs"));

#[derive(Clone)]
pub struct ActorInnerError {
  inner_error: Option<Arc<dyn Any + Send + Sync>>,
  backtrace: Backtrace,
}

impl ActorInnerError {
  pub fn new<T>(inner_error: T) -> Self
  where
    T: Send + Sync + 'static, {
    Self {
      inner_error: Some(Arc::new(inner_error)),
      backtrace: Backtrace::new(),
    }
  }

  pub fn backtrace(&self) -> &Backtrace {
    &self.backtrace
  }

  pub fn is_type<T: Send + Sync + 'static>(&self) -> bool {
    match self.inner_error.as_ref() {
      Some(m) => m.is::<T>(),
      None => false,
    }
  }

  pub fn take<T>(&mut self) -> Result<T, TakeError>
  where
    T: Send + Sync + 'static, {
    match self.inner_error.take() {
      Some(v) => match v.downcast::<T>() {
        Ok(arc_v) => match Arc::try_unwrap(arc_v) {
          Ok(v) => Ok(v),
          Err(arc_v) => {
            self.inner_error = Some(arc_v);
            Err(TakeError::StillShared)
          }
        },
        Err(original) => {
          self.inner_error = Some(original.clone());
          Err(TakeError::TypeMismatch {
            expected: std::any::type_name::<T>(),
            found: original.type_id(),
          })
        }
      },
      None => Err(TakeError::AlreadyTaken),
    }
  }

  pub fn take_or_panic<T>(&mut self) -> T
  where
    T: Error + Send + Sync + 'static, {
    self.take().unwrap_or_else(|e| panic!("Failed to take error: {:?}", e))
  }
}

#[derive(Debug)]
pub enum TakeError {
  TypeMismatch {
    expected: &'static str,
    found: std::any::TypeId,
  },
  StillShared,
  AlreadyTaken,
}

impl Display for ActorInnerError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match &self.inner_error {
      Some(error) => write!(f, "ActorInnerError: {:?}", error),
      None => write!(f, "ActorInnerError: Error has been taken"),
    }
  }
}

impl Debug for ActorInnerError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("ActorInnerError")
      .field("inner_error", &self.inner_error)
      .field("backtrace", &self.backtrace)
      .finish()
  }
}

impl Error for ActorInnerError {}

impl PartialEq for ActorInnerError {
  fn eq(&self, other: &Self) -> bool {
    match (&self.inner_error, &other.inner_error) {
      (Some(a), Some(b)) => Arc::ptr_eq(a, b),
      (None, None) => true,
      _ => false,
    }
  }
}

impl Eq for ActorInnerError {}

impl std::hash::Hash for ActorInnerError {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.inner_error.is_some().hash(state);
    if let Some(error) = &self.inner_error {
      error.type_id().hash(state);
    }
    std::ptr::addr_of!(self.backtrace).hash(state);
  }
}

impl From<std::io::Error> for ActorInnerError {
  fn from(error: std::io::Error) -> Self {
    ActorInnerError {
      inner_error: Some(Arc::new(error)),
      backtrace: Backtrace::new(),
    }
  }
}

impl From<String> for ActorInnerError {
  fn from(s: String) -> Self {
    Self::new(s)
  }
}

impl From<&str> for ActorInnerError {
  fn from(s: &str) -> Self {
    Self::new(s.to_string())
  }
}

#[derive(Debug, Clone)]
pub struct DefaultError {
  msg: String,
}

impl DefaultError {
  pub fn new(msg: String) -> Self {
    DefaultError { msg }
  }
}

impl Display for DefaultError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "DefaultError: {}", self.msg)
  }
}

impl Error for DefaultError {}

#[derive(Debug, Clone)]
pub enum ActorError {
  ReceiveError(ActorInnerError),
  RestartError(ActorInnerError),
  StopError(ActorInnerError),
  InitializationError(ActorInnerError),
  CommunicationError(ActorInnerError),
}

impl ActorError {
  pub fn reason(&self) -> Option<&ActorInnerError> {
    match self {
      ActorError::ReceiveError(e)
      | ActorError::RestartError(e)
      | ActorError::StopError(e)
      | ActorError::InitializationError(e)
      | ActorError::CommunicationError(e) => Some(e),
    }
  }
}

impl Display for ActorError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      ActorError::ReceiveError(e) => write!(f, "Receive error: {}", e),
      ActorError::RestartError(e) => write!(f, "Restart error: {}", e),
      ActorError::StopError(e) => write!(f, "Stop error: {}", e),
      ActorError::InitializationError(e) => write!(f, "Initialization error: {}", e),
      ActorError::CommunicationError(e) => write!(f, "Communication error: {}", e),
    }
  }
}

impl Error for ActorError {
  fn source(&self) -> Option<&(dyn Error + 'static)> {
    match self {
      ActorError::ReceiveError(e)
      | ActorError::RestartError(e)
      | ActorError::StopError(e)
      | ActorError::InitializationError(e)
      | ActorError::CommunicationError(e) => Some(e),
    }
  }
}

#[async_trait]
pub trait Actor: Debug + Send + Sync + 'static {
  async fn handle(&mut self, context_handle: ContextHandle) -> Result<(), ActorError> {
    let id = context_handle.get_self().await.unwrap().id().to_string();
    // tracing::debug!("Actor::handle: id = {}, context_handle = {:?}", id, context_handle);
    let message_handle_opt = context_handle.get_message().await;
    let any_message = message_handle_opt.as_ref().unwrap().as_any();
    if let Some(system_message) = any_message.downcast_ref::<SystemMessage>() {
      // tracing::debug!("Actor::handle: id = {}, system_message = {:?}", id, system_message);
      match system_message {
        SystemMessage::Started(_) => self.started(context_handle).await,
        SystemMessage::Stop(_) => self.stop(context_handle).await,
        SystemMessage::Restart(_) => self.restart(context_handle).await,
      }
    } else if let Some(terminated) = any_message.downcast_ref::<Terminated>() {
      // tracing::debug!("Actor::handle: id = {}, terminated = {:?}", id, terminated);
      self.on_child_terminated(context_handle, terminated).await
    } else if let Some(auto_receive_message) = any_message.downcast_ref::<AutoReceiveMessage>() {
      // tracing::debug!("Actor::handle: id = {}, auto_receive_message = {:?}", id, auto_receive_message);
      match auto_receive_message {
        AutoReceiveMessage::Restarting(_) => self.restarting(context_handle).await,
        AutoReceiveMessage::Stopping(_) => self.stopping(context_handle).await,
        AutoReceiveMessage::Stopped(_) => self.stopped(context_handle).await,
        AutoReceiveMessage::PoisonPill(_) => Ok(()),
      }
    } else {
      // tracing::debug!(
      //   "Actor::handle: id = {}, other = {:?}",
      //   id,
      //   context_handle.get_message().await.unwrap()
      // );
      self
        .receive(context_handle.clone(), context_handle.get_message().await.unwrap())
        .await
    }
  }

  async fn receive(&mut self, c: ContextHandle, message_handle: MessageHandle) -> Result<(), ActorError>;

  async fn started(&self, _: ContextHandle) -> Result<(), ActorError> {
    Ok(())
  }

  async fn stop(&self, _: ContextHandle) -> Result<(), ActorError> {
    Ok(())
  }

  async fn restart(&self, _: ContextHandle) -> Result<(), ActorError> {
    Ok(())
  }

  async fn restarting(&self, _: ContextHandle) -> Result<(), ActorError> {
    Ok(())
  }

  async fn stopping(&self, _: ContextHandle) -> Result<(), ActorError> {
    Ok(())
  }

  async fn stopped(&self, _: ContextHandle) -> Result<(), ActorError> {
    Ok(())
  }

  async fn on_child_terminated(&self, _: ContextHandle, _: &Terminated) -> Result<(), ActorError> {
    Ok(())
  }

  async fn get_supervisor_strategy(&self) -> Option<SupervisorStrategyHandle> {
    None
  }
}

#[derive(Debug, Clone)]
pub struct ActorHandle(Arc<Mutex<dyn Actor>>);

impl PartialEq for ActorHandle {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for ActorHandle {}

impl std::hash::Hash for ActorHandle {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const Mutex<dyn Actor>).hash(state);
  }
}

impl ActorHandle {
  pub fn new_arc(actor: Arc<Mutex<dyn Actor>>) -> Self {
    ActorHandle(actor)
  }

  pub fn new(actor: impl Actor + 'static) -> Self {
    ActorHandle(Arc::new(Mutex::new(actor)))
  }
}

#[async_trait]
impl Actor for ActorHandle {
  async fn handle(&mut self, c: ContextHandle) -> Result<(), ActorError> {
    let mut mg = self.0.lock().await;
    mg.handle(c).await
  }

  async fn receive(&mut self, c: ContextHandle, m: MessageHandle) -> Result<(), ActorError> {
    let mut mg = self.0.lock().await;
    mg.receive(c, m).await
  }

  async fn get_supervisor_strategy(&self) -> Option<SupervisorStrategyHandle> {
    let mg = self.0.lock().await;
    mg.get_supervisor_strategy().await
  }
}
