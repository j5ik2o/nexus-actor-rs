use std::any::Any;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::sync::Arc;

use crate::actor::context::ContextHandle;
use crate::actor::supervisor_strategy::SupervisorStrategyHandle;
use async_trait::async_trait;
use backtrace::Backtrace;

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

impl From<std::io::Error> for ActorInnerError
{
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
pub struct DefaultError{
    msg: String
}

impl DefaultError {
    pub fn new(msg: String) -> Self {
        DefaultError {
            msg
        }
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
  async fn receive(&self, c: ContextHandle);
  fn get_supervisor_strategy(&self) -> Option<SupervisorStrategyHandle> {
    None
  }
}

#[derive(Debug, Clone)]
pub struct ActorHandle(Arc<dyn Actor>);

impl PartialEq for ActorHandle {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for ActorHandle {}

impl std::hash::Hash for ActorHandle {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Actor).hash(state);
  }
}

impl ActorHandle {
  pub fn new(actor: Arc<dyn Actor>) -> Self {
    ActorHandle(actor)
  }
}

#[async_trait]
impl Actor for ActorHandle {
  async fn receive(&self, c: ContextHandle) {
    self.0.receive(c).await
  }
}
