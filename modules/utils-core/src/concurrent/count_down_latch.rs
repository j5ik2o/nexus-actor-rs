use alloc::boxed::Box;
use async_trait::async_trait;

#[async_trait(?Send)]
pub trait CountDownLatchBackend: Clone {
  fn new(count: usize) -> Self;

  async fn count_down(&self);

  async fn wait(&self);
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CountDownLatch<B>
where
  B: CountDownLatchBackend, {
  backend: B,
}

impl<B> CountDownLatch<B>
where
  B: CountDownLatchBackend,
{
  pub fn new(count: usize) -> Self {
    Self { backend: B::new(count) }
  }

  pub async fn count_down(&self) {
    self.backend.count_down().await;
  }

  pub async fn wait(&self) {
    self.backend.wait().await;
  }

  pub fn backend(&self) -> &B {
    &self.backend
  }
}

impl<B> Default for CountDownLatch<B>
where
  B: CountDownLatchBackend,
{
  fn default() -> Self {
    Self::new(0)
  }
}
