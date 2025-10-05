use alloc::boxed::Box;
use core::future::Future;
use core::pin::Pin;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

pub trait CountDownLatchBackend: Clone {
  type CountDownFuture<'a>: Future<Output = ()> + 'a
  where
    Self: 'a;

  type WaitFuture<'a>: Future<Output = ()> + 'a
  where
    Self: 'a;

  fn new(count: usize) -> Self;

  fn count_down(&self) -> Self::CountDownFuture<'_>;

  fn wait(&self) -> Self::WaitFuture<'_>;
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
