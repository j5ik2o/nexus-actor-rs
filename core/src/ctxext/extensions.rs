use async_trait::async_trait;
use std::any::Any;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::actor::{Context, ExtensionPart};

#[derive(Debug)]
pub struct ContextExtensions {
  extensions: Arc<RwLock<Vec<Box<dyn Any + Send + Sync>>>>,
}

impl Context for ContextExtensions {
  fn as_any(&self) -> &dyn Any {
    self
  }
}

#[async_trait]
impl ExtensionPart for ContextExtensions {
  async fn register_extension<T: 'static>(&mut self, extension: T) {
    self.extensions.write().await.push(Box::new(extension));
  }

  async fn get_extension<T: 'static>(&self) -> Option<&T> {
    for ext in self.extensions.read().await.iter() {
      if let Some(ext) = ext.as_any().downcast_ref::<T>() {
        return Some(ext);
      }
    }
    None
  }

  async fn get_extension_mut<T: 'static>(&mut self) -> Option<&mut T> {
    for ext in self.extensions.write().await.iter_mut() {
      if let Some(ext) = ext.as_any_mut().downcast_mut::<T>() {
        return Some(ext);
      }
    }
    None
  }
}

impl ContextExtensions {
  pub fn new() -> Self {
    Self {
      extensions: Arc::new(RwLock::new(Vec::new())),
    }
  }
}
