use crate::actor::context::ExtensionPart;
use async_trait::async_trait;
use nexus_actor_core_rs::context::{
  next_core_context_extension_id, CoreContextExtension, CoreContextExtensionHandle, CoreContextExtensionId,
  CoreContextExtensions, CoreExtensionBorrow, CoreExtensionBorrowMut,
};
use nexus_utils_std_rs::runtime::sync::TokioRwLock;
use std::sync::Arc;

pub type ContextExtensionId = CoreContextExtensionId;

pub trait ContextExtension: CoreContextExtension {}

impl<T> ContextExtension for T where T: CoreContextExtension {}

pub type ContextExtensionHandle = CoreContextExtensionHandle;

#[derive(Debug, Clone)]
pub struct ContextExtensions {
  inner: CoreContextExtensions<TokioRwLock<Vec<Option<ContextExtensionHandle>>>>,
}

impl ContextExtensions {
  pub fn new() -> Self {
    Self {
      inner: CoreContextExtensions::from_lock(Arc::new(TokioRwLock::new(vec![]))),
    }
  }

  pub async fn borrow_extension(&self, id: ContextExtensionId) -> Option<ExtensionBorrow<'_>> {
    self.inner.borrow_extension(id).await.map(ExtensionBorrow)
  }

  pub async fn borrow_extension_mut(&self, id: ContextExtensionId) -> Option<ExtensionBorrowMut<'_>> {
    self.inner.borrow_extension_mut(id).await.map(ExtensionBorrowMut)
  }

  pub async fn inner(&self) -> CoreContextExtensions<TokioRwLock<Vec<Option<ContextExtensionHandle>>>> {
    self.inner.clone()
  }
}

impl Default for ContextExtensions {
  fn default() -> Self {
    Self::new()
  }
}

pub struct ExtensionBorrow<'a>(CoreExtensionBorrow<'a, TokioRwLock<Vec<Option<ContextExtensionHandle>>>>);

impl<'a> ExtensionBorrow<'a> {
  pub fn handle(&self) -> &ContextExtensionHandle {
    self.0.handle()
  }
}

pub struct ExtensionBorrowMut<'a>(CoreExtensionBorrowMut<'a, TokioRwLock<Vec<Option<ContextExtensionHandle>>>>);

impl<'a> ExtensionBorrowMut<'a> {
  pub fn handle(&self) -> &ContextExtensionHandle {
    self.0.handle()
  }

  pub fn handle_mut(&mut self) -> &mut ContextExtensionHandle {
    self.0.handle_mut()
  }
}

#[async_trait]
impl ExtensionPart for ContextExtensions {
  async fn get(&mut self, id: ContextExtensionId) -> Option<ContextExtensionHandle> {
    self.inner.get(id).await
  }

  async fn set(&mut self, extension: ContextExtensionHandle) {
    self.inner.set(extension).await;
  }
}

pub fn next_context_extension_id() -> ContextExtensionId {
  next_core_context_extension_id()
}
