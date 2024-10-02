use crate::actor::context::ExtensionPart;
use async_trait::async_trait;
use std::fmt::Debug;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

pub type ContextExtensionId = i32;

static CURRENT_CONTEXT_EXTENSION_ID: AtomicI32 = AtomicI32::new(0);

pub trait ContextExtension: Debug + Send + Sync + 'static {
  fn extension_id(&self) -> ContextExtensionId;
}

#[derive(Debug, Clone)]
pub struct ContextExtensionHandle(Arc<dyn ContextExtension>);

impl ContextExtensionHandle {
  pub fn new(extension: Arc<dyn ContextExtension>) -> Self {
    ContextExtensionHandle(extension)
  }
}

impl ContextExtension for ContextExtensionHandle {
  fn extension_id(&self) -> ContextExtensionId {
    self.0.extension_id()
  }
}

#[derive(Debug, Clone)]
pub struct ContextExtensions {
  extensions: Arc<Mutex<Vec<Option<ContextExtensionHandle>>>>,
}

impl ContextExtensions {
  pub fn new() -> Self {
    Self {
      extensions: Arc::new(Mutex::new(vec![None, None, None])),
    }
  }
}

impl Default for ContextExtensions {
  fn default() -> Self {
    Self::new()
  }
}

#[async_trait]
impl ExtensionPart for ContextExtensions {
  async fn get(&mut self, id: ContextExtensionId) -> Option<ContextExtensionHandle> {
    let mg = self.extensions.lock().await;
    mg.get(id as usize).and_then(|ext| ext.clone())
  }

  async fn set(&mut self, extension: ContextExtensionHandle) {
    let id = extension.extension_id() as usize;
    let len = {
      let mg = self.extensions.lock().await;
      mg.len()
    };
    if id >= len {
      let len = {
        let mg = self.extensions.lock().await;
        mg.len()
      };
      let mut mg = self.extensions.lock().await;
      mg.extend((len..=id).map(|_| None));
    }
    let mut mg = self.extensions.lock().await;
    mg[id] = Some(extension);
  }
}

pub fn next_context_extension_id() -> ContextExtensionId {
  CURRENT_CONTEXT_EXTENSION_ID.fetch_add(1, Ordering::SeqCst)
}
