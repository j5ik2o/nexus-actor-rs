#![cfg(feature = "alloc")]

use alloc::boxed::Box;
use core::future::Future;
use core::pin::Pin;

use super::message_handle::MessageHandle;
use super::pid::CorePid;
use core::any::Any;

pub type ProcessFuture<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

pub trait CoreProcessHandle: Send + Sync {
  fn send_user_message<'a>(&'a self, pid: Option<&'a CorePid>, message: MessageHandle) -> ProcessFuture<'a>;
  fn send_system_message<'a>(&'a self, pid: &'a CorePid, message: MessageHandle) -> ProcessFuture<'a>;
  fn stop<'a>(&'a self, pid: &'a CorePid) -> ProcessFuture<'a>;
  fn set_dead(&self);
  fn as_any(&self) -> &dyn Any;
}
