#![cfg(feature = "alloc")]

use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt::{Debug, Formatter};
use core::future::Future;
use core::hash::{Hash, Hasher};
use core::pin::Pin;

use super::pid::CorePid;
use super::process::CoreProcessHandle;

pub type CoreProcessRegistryFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub struct CoreAddressResolver<H>
where
  H: CoreProcessHandle + Clone + Send + Sync + 'static, {
  inner: Arc<dyn Fn(&CorePid) -> CoreProcessRegistryFuture<'static, Option<H>> + Send + Sync + 'static>,
}

impl<H> Clone for CoreAddressResolver<H>
where
  H: CoreProcessHandle + Clone + Send + Sync + 'static,
{
  fn clone(&self) -> Self {
    Self {
      inner: Arc::clone(&self.inner),
    }
  }
}

impl<H> CoreAddressResolver<H>
where
  H: CoreProcessHandle + Clone + Send + Sync + 'static,
{
  pub fn new<F, Fut>(resolver: F) -> Self
  where
    F: Fn(&CorePid) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Option<H>> + Send + 'static, {
    let inner =
      Arc::new(move |pid: &CorePid| -> CoreProcessRegistryFuture<'static, Option<H>> { Box::pin(resolver(pid)) });
    Self { inner }
  }

  pub async fn run(&self, pid: &CorePid) -> Option<H> {
    (self.inner)(pid).await
  }
}

impl<H> Debug for CoreAddressResolver<H>
where
  H: CoreProcessHandle + Clone + Send + Sync + 'static,
{
  fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
    write!(f, "CoreAddressResolver")
  }
}

impl<H> PartialEq for CoreAddressResolver<H>
where
  H: CoreProcessHandle + Clone + Send + Sync + 'static,
{
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.inner, &other.inner)
  }
}

impl<H> Eq for CoreAddressResolver<H> where H: CoreProcessHandle + Clone + Send + Sync + 'static {}

impl<H> Hash for CoreAddressResolver<H>
where
  H: CoreProcessHandle + Clone + Send + Sync + 'static,
{
  fn hash<T: Hasher>(&self, state: &mut T) {
    Arc::as_ptr(&self.inner).hash(state);
  }
}

pub trait CoreProcessRegistry: Send + Sync {
  type Handle: CoreProcessHandle + Clone + Send + Sync + 'static;

  fn register_address_resolver(&self, resolver: CoreAddressResolver<Self::Handle>);
  fn set_address(&self, address: alloc::string::String);
  fn get_address(&self) -> alloc::string::String;
  fn next_id(&self) -> alloc::string::String;

  fn list_local_pids<'a>(&'a self) -> CoreProcessRegistryFuture<'a, Vec<CorePid>>;
  fn find_local_process_handle<'a>(&'a self, id: &'a str) -> CoreProcessRegistryFuture<'a, Option<Self::Handle>>;
  fn add_process<'a>(&'a self, process: Self::Handle, id: &'a str) -> CoreProcessRegistryFuture<'a, (CorePid, bool)>;
  fn remove_process<'a>(&'a self, pid: &'a CorePid) -> CoreProcessRegistryFuture<'a, ()>;
  fn get_process<'a>(&'a self, pid: &'a CorePid) -> CoreProcessRegistryFuture<'a, Option<Self::Handle>>;
  fn get_local_process<'a>(&'a self, id: &'a str) -> CoreProcessRegistryFuture<'a, Option<Self::Handle>>;
}
