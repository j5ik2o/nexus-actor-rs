use crate::actor::context::ExtensionPart;
use async_trait::async_trait;
use std::fmt::Debug;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

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
  extensions: Arc<RwLock<Vec<Option<ContextExtensionHandle>>>>,
}

impl ContextExtensions {
  pub fn new() -> Self {
    Self {
      extensions: Arc::new(RwLock::new(vec![None, None, None])),
    }
  }

  pub async fn borrow_extension(&self, id: ContextExtensionId) -> Option<ExtensionBorrow<'_>> {
    let guard = self.extensions.read().await;
    if let Some(Some(_)) = guard.get(id as usize) {
      Some(ExtensionBorrow {
        guard,
        index: id as usize,
      })
    } else {
      None
    }
  }

  pub async fn borrow_extension_mut(&self, id: ContextExtensionId) -> Option<ExtensionBorrowMut<'_>> {
    let guard = self.extensions.write().await;
    if let Some(Some(_)) = guard.get(id as usize) {
      Some(ExtensionBorrowMut {
        guard,
        index: id as usize,
      })
    } else {
      None
    }
  }
}

impl Default for ContextExtensions {
  fn default() -> Self {
    Self::new()
  }
}

pub struct ExtensionBorrow<'a> {
  guard: RwLockReadGuard<'a, Vec<Option<ContextExtensionHandle>>>,
  index: usize,
}

impl<'a> ExtensionBorrow<'a> {
  pub fn handle(&self) -> &ContextExtensionHandle {
    self.guard[self.index].as_ref().expect("extension missing")
  }
}

pub struct ExtensionBorrowMut<'a> {
  guard: RwLockWriteGuard<'a, Vec<Option<ContextExtensionHandle>>>,
  index: usize,
}

impl<'a> ExtensionBorrowMut<'a> {
  pub fn handle(&self) -> &ContextExtensionHandle {
    self.guard[self.index].as_ref().expect("extension missing")
  }

  pub fn handle_mut(&mut self) -> &mut ContextExtensionHandle {
    self.guard[self.index].as_mut().expect("extension missing")
  }
}

#[async_trait]
impl ExtensionPart for ContextExtensions {
  async fn get(&mut self, id: ContextExtensionId) -> Option<ContextExtensionHandle> {
    let mg = self.extensions.read().await;
    mg.get(id as usize).and_then(|ext| ext.clone())
  }

  async fn set(&mut self, extension: ContextExtensionHandle) {
    let id = extension.extension_id() as usize;
    let mut guard = self.extensions.write().await;
    if id >= guard.len() {
      let missing = id + 1 - guard.len();
      guard.extend(std::iter::repeat(None).take(missing));
    }
    guard[id] = Some(extension);
  }
}

pub fn next_context_extension_id() -> ContextExtensionId {
  CURRENT_CONTEXT_EXTENSION_ID.fetch_add(1, Ordering::SeqCst)
}
