#![cfg(feature = "alloc")]

use alloc::boxed::Box;
use core::future::Future;
use core::pin::Pin;

use super::message_handle::MessageHandle;

/// 汎用 Mailbox 操作用の Future 型。
pub type CoreMailboxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
/// キュー操作を Future で扱うための汎用型。
pub type CoreMailboxQueueFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// メールボックスの裏側にあるキュー構造に求める最小限の操作集合。
pub trait CoreMailboxQueue: Send + Sync {
  type Error;

  fn offer(&self, message: MessageHandle) -> Result<(), Self::Error>;
  fn poll(&self) -> Result<Option<MessageHandle>, Self::Error>;
  fn len(&self) -> usize;
  fn clean_up(&self);
}

/// Mailbox に対する最小限の操作集合。no_std + alloc 環境でも扱えるよう、
/// 実装側に `async_trait` などの依存を課さず `Future` を返すインターフェースとする。
pub trait CoreMailbox: Send + Sync {
  fn post_user_message<'a>(&'a self, message: MessageHandle) -> CoreMailboxFuture<'a, ()>;
  fn post_system_message<'a>(&'a self, message: MessageHandle) -> CoreMailboxFuture<'a, ()>;
  fn process_messages<'a>(&'a self) -> CoreMailboxFuture<'a, ()>;
  fn start<'a>(&'a self) -> CoreMailboxFuture<'a, ()>;
  fn user_messages_count<'a>(&'a self) -> CoreMailboxFuture<'a, i32>;
  fn system_messages_count<'a>(&'a self) -> CoreMailboxFuture<'a, i32>;
}

pub trait CoreMailboxQueueAsyncExt {
  type Error;

  fn offer_async(&self, message: MessageHandle) -> CoreMailboxQueueFuture<'_, Result<(), Self::Error>>;
  fn poll_async(&self) -> CoreMailboxQueueFuture<'_, Result<Option<MessageHandle>, Self::Error>>;
  fn clean_up_async(&self) -> CoreMailboxQueueFuture<'_, ()>;
}

impl<T> CoreMailboxQueueAsyncExt for T
where
  T: CoreMailboxQueue,
{
  type Error = T::Error;

  fn offer_async(&self, message: MessageHandle) -> CoreMailboxQueueFuture<'_, Result<(), Self::Error>> {
    Box::pin(async move { self.offer(message) })
  }

  fn poll_async(&self) -> CoreMailboxQueueFuture<'_, Result<Option<MessageHandle>, Self::Error>> {
    Box::pin(async move { self.poll() })
  }

  fn clean_up_async(&self) -> CoreMailboxQueueFuture<'_, ()> {
    Box::pin(async move {
      self.clean_up();
    })
  }
}
