use std::any::Any;
use std::fmt::Debug;
use std::sync::Arc;

pub trait NotInfluenceReceiveTimeout: Debug + Send + Sync + 'static {
  fn as_any(&self) -> &dyn Any;
  fn not_influence_receive_timeout(&self);
}

#[derive(Debug, Clone)]
pub struct NotInfluenceReceiveTimeoutHandle(pub Arc<dyn NotInfluenceReceiveTimeout>);
