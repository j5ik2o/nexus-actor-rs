#![cfg(feature = "alloc")]

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt::{Debug, Formatter};
use core::sync::atomic::{AtomicI32, Ordering};

use crate::runtime::AsyncRwLock;

pub type CoreContextExtensionId = i32;

static NEXT_CONTEXT_EXTENSION_ID: AtomicI32 = AtomicI32::new(0);

#[must_use]
pub fn next_core_context_extension_id() -> CoreContextExtensionId {
  NEXT_CONTEXT_EXTENSION_ID.fetch_add(1, Ordering::SeqCst)
}

pub trait CoreContextExtension: Debug + Send + Sync + 'static {
  fn extension_id(&self) -> CoreContextExtensionId;
}

#[derive(Clone)]
pub struct CoreContextExtensionHandle(Arc<dyn CoreContextExtension>);

impl CoreContextExtensionHandle {
  #[must_use]
  pub fn new(extension: Arc<dyn CoreContextExtension>) -> Self {
    Self(extension)
  }

  #[must_use]
  pub fn inner(&self) -> &Arc<dyn CoreContextExtension> {
    &self.0
  }
}

impl Debug for CoreContextExtensionHandle {
  fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("CoreContextExtensionHandle")
      .field("id", &self.extension_id())
      .finish()
  }
}

impl CoreContextExtension for CoreContextExtensionHandle {
  fn extension_id(&self) -> CoreContextExtensionId {
    self.0.extension_id()
  }
}

pub struct CoreContextExtensions<L>
where
  L: AsyncRwLock<Vec<Option<CoreContextExtensionHandle>>> + Send + Sync + 'static, {
  extensions: Arc<L>,
}

impl<L> CoreContextExtensions<L>
where
  L: AsyncRwLock<Vec<Option<CoreContextExtensionHandle>>> + Default + Send + Sync + 'static,
{
  #[must_use]
  pub fn new() -> Self {
    Self {
      extensions: Arc::new(L::default()),
    }
  }
}

impl<L> CoreContextExtensions<L>
where
  L: AsyncRwLock<Vec<Option<CoreContextExtensionHandle>>> + Send + Sync + 'static,
{
  #[must_use]
  pub fn from_lock(lock: Arc<L>) -> Self {
    Self { extensions: lock }
  }

  pub async fn borrow_extension(&self, id: CoreContextExtensionId) -> Option<CoreExtensionBorrow<'_, L>> {
    if id < 0 {
      return None;
    }
    let guard = self.extensions.read().await;
    match guard.get(id as usize) {
      Some(Some(_)) => Some(CoreExtensionBorrow {
        guard,
        index: id as usize,
      }),
      _ => None,
    }
  }

  pub async fn borrow_extension_mut(&self, id: CoreContextExtensionId) -> Option<CoreExtensionBorrowMut<'_, L>> {
    if id < 0 {
      return None;
    }
    let guard = self.extensions.write().await;
    match guard.get(id as usize) {
      Some(Some(_)) => Some(CoreExtensionBorrowMut {
        guard,
        index: id as usize,
      }),
      _ => None,
    }
  }

  pub async fn get(&self, id: CoreContextExtensionId) -> Option<CoreContextExtensionHandle> {
    if id < 0 {
      return None;
    }
    let guard = self.extensions.read().await;
    guard.get(id as usize).and_then(|ext| ext.clone())
  }

  pub async fn set(&self, extension: CoreContextExtensionHandle) {
    let id = extension.extension_id();
    if id < 0 {
      return;
    }
    let index = id as usize;
    let mut guard = self.extensions.write().await;
    let len = guard.len();
    if index >= len {
      guard.extend(core::iter::repeat(None).take(index + 1 - len));
    }
    guard[index] = Some(extension);
  }
}

pub struct CoreExtensionBorrow<'a, L>
where
  L: AsyncRwLock<Vec<Option<CoreContextExtensionHandle>>> + Send + Sync + 'static, {
  guard: L::ReadGuard<'a>,
  index: usize,
}

impl<'a, L> CoreExtensionBorrow<'a, L>
where
  L: AsyncRwLock<Vec<Option<CoreContextExtensionHandle>>> + Send + Sync + 'static,
{
  #[must_use]
  pub fn handle(&self) -> &CoreContextExtensionHandle {
    self.guard[self.index].as_ref().expect("extension missing")
  }
}

pub struct CoreExtensionBorrowMut<'a, L>
where
  L: AsyncRwLock<Vec<Option<CoreContextExtensionHandle>>> + Send + Sync + 'static, {
  guard: L::WriteGuard<'a>,
  index: usize,
}

impl<'a, L> CoreExtensionBorrowMut<'a, L>
where
  L: AsyncRwLock<Vec<Option<CoreContextExtensionHandle>>> + Send + Sync + 'static,
{
  #[must_use]
  pub fn handle(&self) -> &CoreContextExtensionHandle {
    self.guard[self.index].as_ref().expect("extension missing")
  }

  #[must_use]
  pub fn handle_mut(&mut self) -> &mut CoreContextExtensionHandle {
    self.guard[self.index].as_mut().expect("extension missing")
  }
}

impl<L> Clone for CoreContextExtensions<L>
where
  L: AsyncRwLock<Vec<Option<CoreContextExtensionHandle>>> + Send + Sync + 'static,
{
  fn clone(&self) -> Self {
    Self {
      extensions: Arc::clone(&self.extensions),
    }
  }
}

impl<L> Debug for CoreContextExtensions<L>
where
  L: AsyncRwLock<Vec<Option<CoreContextExtensionHandle>>> + Send + Sync + 'static,
{
  fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("CoreContextExtensions")
      .field("ptr", &Arc::as_ptr(&self.extensions))
      .finish()
  }
}
