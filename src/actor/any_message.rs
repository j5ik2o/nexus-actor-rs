use std::any::Any;
use std::fmt;
use std::fmt::Debug;
use std::sync::Arc;

use crate::core::dispatch::message::Message;
use crate::core::util::element::Element;

#[derive(Debug)]
pub enum AnyMessageError {
  Downcast,
  ArcTryUnwrap,
}

pub struct AnyMessage {
  pub msg: Option<Arc<dyn Any + Send + Sync>>,
}

unsafe impl Send for AnyMessage {}
unsafe impl Sync for AnyMessage {}

impl Element for AnyMessage {}

impl AnyMessage {
  pub fn new<T>(msg: T) -> Self
  where
    T: Any + Message + Send + Sync + 'static, {
    Self {
      msg: Some(Arc::new(msg)),
    }
  }

  pub fn is_type<T: 'static>(&self) -> bool {
    match self.msg.as_ref() {
      Some(m) => m.is::<T>(),
      None => false,
    }
  }

  pub fn take<T>(&mut self) -> Result<T, AnyMessageError>
  where
    T: Any + Message + Send + Sync, {
    match self.msg.take() {
      Some(m) => {
        if m.is::<T>() {
          let data = Arc::try_unwrap(m.downcast::<T>().unwrap()).map_err(|_| AnyMessageError::ArcTryUnwrap)?;
          Ok(data)
        } else {
          Err(AnyMessageError::Downcast)
        }
      }
      None => Err(AnyMessageError::Downcast),
    }
  }
}

impl Clone for AnyMessage {
  fn clone(&self) -> Self {
    Self { msg: self.msg.clone() }
  }
}

impl Debug for AnyMessage {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "AnyMessage({:?})", self.msg)
  }
}

impl PartialEq for AnyMessage {
  fn eq(&self, other: &Self) -> bool {
    match (&self.msg, &other.msg) {
      (Some(a), Some(b)) => Arc::ptr_eq(a, b),
      (None, None) => true,
      _ => false,
    }
  }
}
