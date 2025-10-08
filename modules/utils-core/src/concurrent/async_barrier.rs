use alloc::boxed::Box;
use async_trait::async_trait;

#[async_trait(?Send)]
pub trait AsyncBarrierBackend: Clone {
  fn new(count: usize) -> Self;

  async fn wait(&self);
}

#[derive(Clone, Debug)]
pub struct AsyncBarrier<B>
where
  B: AsyncBarrierBackend, {
  backend: B,
}

impl<B> AsyncBarrier<B>
where
  B: AsyncBarrierBackend,
{
  pub fn new(count: usize) -> Self {
    Self { backend: B::new(count) }
  }

  pub async fn wait(&self) {
    self.backend.wait().await;
  }

  pub fn backend(&self) -> &B {
    &self.backend
  }
}
