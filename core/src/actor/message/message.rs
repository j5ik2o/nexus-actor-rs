use std::any::Any;
use std::fmt::Debug;

pub trait Message: Debug + Send + Sync + 'static {
  fn as_any(&self) -> &(dyn Any + Send + Sync);
  fn message_type(&self) -> &'static str {
    std::any::type_name::<Self>()
  }
}

// Implement for all types that implement the required traits
impl<T: Debug + Send + Sync + 'static + PartialEq> Message for T {
  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }
}
