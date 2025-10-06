use alloc::boxed::Box;
use core::future::Future;
use core::pin::Pin;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

pub trait WaitGroupBackend: Clone {
  fn new() -> Self;
  fn with_count(count: usize) -> Self;
  fn add(&self, n: usize);
  fn done(&self);
  type WaitFuture<'a>: Future<Output = ()> + 'a
  where
    Self: 'a;
  fn wait(&self) -> Self::WaitFuture<'_>;
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
