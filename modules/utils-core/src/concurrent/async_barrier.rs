use alloc::boxed::Box;
use core::future::Future;
use core::pin::Pin;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

pub trait AsyncBarrierBackend: Clone {
  type WaitFuture<'a>: Future<Output = ()> + 'a
  where
    Self: 'a;

  fn new(count: usize) -> Self;

  fn wait(&self) -> Self::WaitFuture<'_>;
}

#[derive(Clone, Debug)]
pub struct AsyncBarrier<B>
where
  B: AsyncBarrierBackend,
{
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
