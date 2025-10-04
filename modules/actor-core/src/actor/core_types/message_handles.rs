#![cfg(feature = "alloc")]

use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::actor::core_types::message_handle::MessageHandle;
use crate::runtime::AsyncMutex;

/// コアレイヤで利用するメッセージハンドルコレクション。
/// 具体的な `AsyncMutex` 実装は呼び出し側（std など）が注入する。
pub struct CoreMessageHandles<M>
where
  M: AsyncMutex<Vec<MessageHandle>> + Send + Sync + 'static, {
  inner: Arc<M>,
}

impl<M> CoreMessageHandles<M>
where
  M: AsyncMutex<Vec<MessageHandle>> + Send + Sync + 'static,
{
  /// 任意の `AsyncMutex` 実装からメッセージハンドルコレクションを構築する。
  #[must_use]
  pub fn from_mutex(mutex: M) -> Self {
    Self::from_arc(Arc::new(mutex))
  }

  /// 共有参照を直接注入するコンストラクタ。
  #[must_use]
  pub fn from_arc(inner: Arc<M>) -> Self {
    Self { inner }
  }

  pub async fn push(&self, handle: MessageHandle) {
    self.inner.lock().await.push(handle);
  }

  pub async fn pop(&self) -> Option<MessageHandle> {
    self.inner.lock().await.pop()
  }

  pub async fn len(&self) -> usize {
    self.inner.lock().await.len()
  }

  pub async fn is_empty(&self) -> bool {
    self.len().await == 0
  }

  pub async fn clear(&self) {
    self.inner.lock().await.clear();
  }

  pub async fn to_vec(&self) -> Vec<MessageHandle> {
    self.inner.lock().await.clone()
  }

  #[must_use]
  pub fn inner(&self) -> Arc<M> {
    self.inner.clone()
  }
}

impl<M> Clone for CoreMessageHandles<M>
where
  M: AsyncMutex<Vec<MessageHandle>> + Send + Sync + 'static,
{
  fn clone(&self) -> Self {
    Self {
      inner: Arc::clone(&self.inner),
    }
  }
}

impl<M> core::fmt::Debug for CoreMessageHandles<M>
where
  M: AsyncMutex<Vec<MessageHandle>> + Send + Sync + 'static,
{
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("CoreMessageHandles")
      .field("ptr", &Arc::as_ptr(&self.inner))
      .finish()
  }
}

impl<M> Default for CoreMessageHandles<M>
where
  M: AsyncMutex<Vec<MessageHandle>> + Default + Send + Sync + 'static,
{
  fn default() -> Self {
    Self::from_mutex(M::default())
  }
}
