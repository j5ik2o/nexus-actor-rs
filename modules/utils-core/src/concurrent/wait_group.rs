use alloc::boxed::Box;
use async_trait::async_trait;

#[async_trait(?Send)]
pub trait WaitGroupBackend: Clone {
  fn new() -> Self;
  fn with_count(count: usize) -> Self;
  fn add(&self, n: usize);
  fn done(&self);
  async fn wait(&self);
}

#[derive(Clone, Debug)]
pub struct WaitGroup<B>
where
  B: WaitGroupBackend, {
  backend: B,
}

impl<B> WaitGroup<B>
where
  B: WaitGroupBackend,
{
  pub fn new() -> Self {
    Self { backend: B::new() }
  }

  pub fn with_count(count: usize) -> Self {
    Self {
      backend: B::with_count(count),
    }
  }

  pub fn add(&self, n: usize) {
    self.backend.add(n);
  }

  pub fn done(&self) {
    self.backend.done();
  }

  pub async fn wait(&self) {
    self.backend.wait().await;
  }

  pub fn backend(&self) -> &B {
    &self.backend
  }
}

impl<B> Default for WaitGroup<B>
where
  B: WaitGroupBackend,
{
  fn default() -> Self {
    Self::new()
  }
}
