#![cfg(feature = "alloc")]

use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt::{Debug, Formatter};
use core::future::Future;
use core::hash::{Hash, Hasher};
use core::pin::Pin;

/// コアレイヤで共有する `Future` 型のエイリアス。
pub type CoreFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Receiver middleware チェーン内部で利用する非同期関数型。
pub type CoreReceiverAsyncFn<S, E> = Arc<dyn Fn(S) -> CoreFuture<'static, Result<(), E>> + Send + Sync>;
/// Receiver middleware チェーン内部で利用する同期関数型。
pub type CoreReceiverSyncFn<S> = Arc<dyn Fn(S) -> S + Send + Sync + 'static>;

/// Receiver middleware の合成チェーン。
#[derive(Clone)]
pub struct CoreReceiverMiddlewareChain<S, E>
where
  S: Send + 'static,
  E: Send + 'static, {
  sync_step: CoreReceiverSyncFn<S>,
  async_step: CoreReceiverAsyncFn<S, E>,
}

impl<S, E> CoreReceiverMiddlewareChain<S, E>
where
  S: Send + 'static,
  E: Send + 'static,
{
  /// 最終段 (tail) の非同期ハンドラからチェーンを構築する。
  pub fn new<F, Fut>(tail: F) -> Self
  where
    F: Fn(S) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<(), E>> + Send + 'static, {
    let async_step: CoreReceiverAsyncFn<S, E> = Arc::new(move |snapshot| {
      let fut = tail(snapshot);
      Box::pin(fut) as CoreFuture<'static, Result<(), E>>
    });

    Self {
      sync_step: Arc::new(|snapshot| snapshot),
      async_step,
    }
  }

  /// 逐次実行される同期処理を合成する。
  pub fn with_sync<F>(self, sync: F) -> Self
  where
    F: Fn(S) -> S + Send + Sync + 'static,
    S: 'static, {
    let previous = self.sync_step.clone();
    Self {
      sync_step: Arc::new(move |snapshot| {
        let after_prev = previous(snapshot);
        sync(after_prev)
      }),
      async_step: self.async_step.clone(),
    }
  }

  /// 非同期処理をラップして合成する。
  pub fn with_async<F>(self, wrapper: F) -> Self
  where
    F: Fn(S, CoreReceiverAsyncFn<S, E>) -> CoreFuture<'static, Result<(), E>> + Send + Sync + 'static,
    S: Clone + 'static, {
    let async_prev = self.async_step.clone();
    Self {
      sync_step: self.sync_step.clone(),
      async_step: Arc::new(move |snapshot| wrapper(snapshot, async_prev.clone())),
    }
  }

  /// 同期ステップを適用する。
  pub fn apply_sync(&self, snapshot: S) -> S {
    (self.sync_step)(snapshot)
  }

  /// 非同期ステップを実行する。
  pub fn call_async(&self, snapshot: S) -> CoreFuture<'static, Result<(), E>> {
    (self.async_step)(snapshot)
  }
}

impl<S, E> Debug for CoreReceiverMiddlewareChain<S, E>
where
  S: Send + 'static,
  E: Send + 'static,
{
  fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
    write!(f, "CoreReceiverMiddlewareChain")
  }
}

impl<S, E> PartialEq for CoreReceiverMiddlewareChain<S, E>
where
  S: Send + 'static,
  E: Send + 'static,
{
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.sync_step, &other.sync_step) && Arc::ptr_eq(&self.async_step, &other.async_step)
  }
}

impl<S, E> Eq for CoreReceiverMiddlewareChain<S, E>
where
  S: Send + 'static,
  E: Send + 'static,
{
}

impl<S, E> Hash for CoreReceiverMiddlewareChain<S, E>
where
  S: Send + 'static,
  E: Send + 'static,
{
  fn hash<H: Hasher>(&self, state: &mut H) {
    (Arc::as_ptr(&self.sync_step) as *const ()).hash(state);
    (Arc::as_ptr(&self.async_step) as *const ()).hash(state);
  }
}

/// receiver middleware の関数型。
#[derive(Clone)]
pub struct CoreReceiverMiddleware<S, E>
where
  S: Send + 'static,
  E: Send + 'static, {
  inner: Arc<dyn Fn(CoreReceiverMiddlewareChain<S, E>) -> CoreReceiverMiddlewareChain<S, E> + Send + Sync + 'static>,
}

impl<S, E> CoreReceiverMiddleware<S, E>
where
  S: Send + 'static,
  E: Send + 'static,
{
  pub fn new<F>(f: F) -> Self
  where
    F: Fn(CoreReceiverMiddlewareChain<S, E>) -> CoreReceiverMiddlewareChain<S, E> + Send + Sync + 'static, {
    Self { inner: Arc::new(f) }
  }

  pub fn from_sync<F>(f: F) -> Self
  where
    F: Fn(S) -> S + Send + Sync + 'static,
    S: Clone + 'static, {
    let arc = Arc::new(f);
    Self::new(move |chain| {
      let arc = arc.clone();
      chain.with_sync(move |snapshot| arc(snapshot))
    })
  }

  pub fn from_async<F>(f: F) -> Self
  where
    F: Fn(S, CoreReceiverAsyncFn<S, E>) -> CoreFuture<'static, Result<(), E>> + Send + Sync + 'static,
    S: Clone + 'static, {
    let arc = Arc::new(f);
    Self::new(move |chain| {
      let arc = arc.clone();
      chain.with_async(move |snapshot, next| arc(snapshot, next))
    })
  }

  pub fn run(&self, next: CoreReceiverMiddlewareChain<S, E>) -> CoreReceiverMiddlewareChain<S, E> {
    (self.inner)(next)
  }
}

impl<S, E> Debug for CoreReceiverMiddleware<S, E>
where
  S: Send + 'static,
  E: Send + 'static,
{
  fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
    write!(f, "CoreReceiverMiddleware")
  }
}

impl<S, E> PartialEq for CoreReceiverMiddleware<S, E>
where
  S: Send + 'static,
  E: Send + 'static,
{
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.inner, &other.inner)
  }
}

impl<S, E> Eq for CoreReceiverMiddleware<S, E>
where
  S: Send + 'static,
  E: Send + 'static,
{
}

impl<S, E> Hash for CoreReceiverMiddleware<S, E>
where
  S: Send + 'static,
  E: Send + 'static,
{
  fn hash<H: Hasher>(&self, state: &mut H) {
    (Arc::as_ptr(&self.inner) as *const ()).hash(state);
  }
}

/// Sender middleware の非同期関数型。
pub type CoreSenderAsyncFn<A> = Arc<dyn Fn(A) -> CoreFuture<'static, ()> + Send + Sync>;

/// Sender middleware チェーン。
#[derive(Clone)]
pub struct CoreSenderMiddlewareChain<A>
where
  A: Send + 'static, {
  async_step: CoreSenderAsyncFn<A>,
}

impl<A> CoreSenderMiddlewareChain<A>
where
  A: Send + 'static,
{
  pub fn new<F, Fut>(f: F) -> Self
  where
    F: Fn(A) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static, {
    Self {
      async_step: Arc::new(move |args| Box::pin(f(args)) as CoreFuture<'static, ()>),
    }
  }

  pub async fn run(&self, args: A) {
    (self.async_step)(args).await
  }
}

impl<A> Debug for CoreSenderMiddlewareChain<A>
where
  A: Send + 'static,
{
  fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
    write!(f, "CoreSenderMiddlewareChain")
  }
}

impl<A> PartialEq for CoreSenderMiddlewareChain<A>
where
  A: Send + 'static,
{
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.async_step, &other.async_step)
  }
}

impl<A> Eq for CoreSenderMiddlewareChain<A> where A: Send + 'static {}

impl<A> Hash for CoreSenderMiddlewareChain<A>
where
  A: Send + 'static,
{
  fn hash<H: Hasher>(&self, state: &mut H) {
    (Arc::as_ptr(&self.async_step) as *const ()).hash(state);
  }
}

#[derive(Clone)]
pub struct CoreSenderMiddleware<A>
where
  A: Send + 'static, {
  inner: Arc<dyn Fn(CoreSenderMiddlewareChain<A>) -> CoreSenderMiddlewareChain<A> + Send + Sync + 'static>,
}

impl<A> CoreSenderMiddleware<A>
where
  A: Send + 'static,
{
  pub fn new<F>(f: F) -> Self
  where
    F: Fn(CoreSenderMiddlewareChain<A>) -> CoreSenderMiddlewareChain<A> + Send + Sync + 'static, {
    Self { inner: Arc::new(f) }
  }

  pub fn run(&self, next: CoreSenderMiddlewareChain<A>) -> CoreSenderMiddlewareChain<A> {
    (self.inner)(next)
  }
}

impl<A> Debug for CoreSenderMiddleware<A>
where
  A: Send + 'static,
{
  fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
    write!(f, "CoreSenderMiddleware")
  }
}

impl<A> PartialEq for CoreSenderMiddleware<A>
where
  A: Send + 'static,
{
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.inner, &other.inner)
  }
}

impl<A> Eq for CoreSenderMiddleware<A> where A: Send + 'static {}

impl<A> Hash for CoreSenderMiddleware<A>
where
  A: Send + 'static,
{
  fn hash<H: Hasher>(&self, state: &mut H) {
    (Arc::as_ptr(&self.inner) as *const ()).hash(state);
  }
}

/// Spawn middleware は同期変換のみを扱う。
#[derive(Clone)]
pub struct CoreSpawnMiddleware<T>
where
  T: Send + 'static, {
  inner: Arc<dyn Fn(T) -> T + Send + Sync + 'static>,
}

impl<T> CoreSpawnMiddleware<T>
where
  T: Send + 'static,
{
  pub fn new<F>(f: F) -> Self
  where
    F: Fn(T) -> T + Send + Sync + 'static, {
    Self { inner: Arc::new(f) }
  }

  pub fn run(&self, next: T) -> T {
    (self.inner)(next)
  }
}

impl<T> Debug for CoreSpawnMiddleware<T>
where
  T: Send + 'static,
{
  fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
    write!(f, "CoreSpawnMiddleware")
  }
}

impl<T> PartialEq for CoreSpawnMiddleware<T>
where
  T: Send + 'static,
{
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.inner, &other.inner)
  }
}

impl<T> Eq for CoreSpawnMiddleware<T> where T: Send + 'static {}

impl<T> Hash for CoreSpawnMiddleware<T>
where
  T: Send + 'static,
{
  fn hash<H: Hasher>(&self, state: &mut H) {
    (Arc::as_ptr(&self.inner) as *const ()).hash(state);
  }
}

static_assertions::assert_impl_all!(
  CoreReceiverMiddlewareChain<u8, ()>: Send, Sync
);
static_assertions::assert_impl_all!(CoreSenderMiddlewareChain<u8>: Send, Sync);
static_assertions::assert_impl_all!(CoreSpawnMiddleware<u8>: Send, Sync);

/// 与えられた receiver middleware 群からチェーンを構築する。
pub fn compose_receiver_chain<'a, S, E, I>(
  middlewares: I,
  tail: CoreReceiverMiddlewareChain<S, E>,
) -> Option<CoreReceiverMiddlewareChain<S, E>>
where
  S: Send + 'static,
  E: Send + 'static,
  I: IntoIterator<Item = &'a CoreReceiverMiddleware<S, E>>, {
  let collected: Vec<_> = middlewares.into_iter().collect();
  if collected.is_empty() {
    return None;
  }

  let mut chain = tail;
  for middleware in collected.into_iter().rev() {
    chain = middleware.run(chain);
  }

  Some(chain)
}

/// 与えられた sender middleware 群からチェーンを構築する。
pub fn compose_sender_chain<'a, A, I>(
  middlewares: I,
  tail: CoreSenderMiddlewareChain<A>,
) -> Option<CoreSenderMiddlewareChain<A>>
where
  A: Send + 'static,
  I: IntoIterator<Item = &'a CoreSenderMiddleware<A>>, {
  let collected: Vec<_> = middlewares.into_iter().collect();
  if collected.is_empty() {
    return None;
  }

  let mut chain = tail;
  for middleware in collected.into_iter().rev() {
    chain = middleware.run(chain);
  }

  Some(chain)
}

/// 与えられた spawn middleware 群を適用し、最終的な Spawner（あるいは同等の型）を返す。
pub fn compose_spawn_chain<'a, T, I>(middlewares: I, tail: T) -> Option<T>
where
  T: Send + 'static,
  I: IntoIterator<Item = &'a CoreSpawnMiddleware<T>>, {
  let collected: Vec<_> = middlewares.into_iter().collect();
  if collected.is_empty() {
    return None;
  }

  let mut current = tail;
  for middleware in collected.into_iter().rev() {
    current = middleware.run(current);
  }

  Some(current)
}
