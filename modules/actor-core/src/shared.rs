#[cfg(not(target_has_atomic = "ptr"))]
use alloc::rc::Rc as Arc;
#[cfg(target_has_atomic = "ptr")]
use alloc::sync::Arc;
use core::ops::Deref;

use crate::runtime::scheduler::ReceiveTimeoutSchedulerFactory;
use crate::{FailureEvent, FailureInfo, MailboxFactory, PriorityEnvelope, SystemMessage};
use nexus_utils_core_rs::sync::{ArcShared, SharedBound};
use nexus_utils_core_rs::Element;

#[cfg(target_has_atomic = "ptr")]
type MapSystemFn<M> = dyn Fn(SystemMessage) -> M + Send + Sync;

#[cfg(not(target_has_atomic = "ptr"))]
type MapSystemFn<M> = dyn Fn(SystemMessage) -> M;

#[cfg(target_has_atomic = "ptr")]
type FailureEventHandlerFn = dyn Fn(&FailureInfo) + Send + Sync;

#[cfg(not(target_has_atomic = "ptr"))]
type FailureEventHandlerFn = dyn Fn(&FailureInfo);

#[cfg(target_has_atomic = "ptr")]
type FailureEventListenerFn = dyn Fn(FailureEvent) + Send + Sync;

#[cfg(not(target_has_atomic = "ptr"))]
type FailureEventListenerFn = dyn Fn(FailureEvent);

/// Shared handle to a system message mapper function.
///
/// Internally stores the mapper inside a `Shared` abstraction so that
/// different backends (`Arc`, `Rc`, etc.) can be plugged in later without
/// touching the call sites in `actor-core`.
pub struct MapSystemShared<M> {
  inner: ArcShared<MapSystemFn<M>>,
}

impl<M> MapSystemShared<M> {
  /// Creates a new shared mapper from a function or closure.
  pub fn new<F>(f: F) -> Self
  where
    F: Fn(SystemMessage) -> M + SharedBound + 'static, {
    Self {
      inner: ArcShared::from_arc(Arc::new(f)),
    }
  }

  /// Wraps an existing shared mapper.
  pub fn from_shared(inner: ArcShared<MapSystemFn<M>>) -> Self {
    Self { inner }
  }

  /// Consumes the wrapper and returns the underlying `Arc`.
  pub fn into_arc(self) -> Arc<MapSystemFn<M>> {
    self.inner.into_arc()
  }

  /// Returns the inner shared handle.
  pub fn as_shared(&self) -> &ArcShared<MapSystemFn<M>> {
    &self.inner
  }
}

impl<M> Clone for MapSystemShared<M> {
  fn clone(&self) -> Self {
    Self {
      inner: self.inner.clone(),
    }
  }
}

impl<M> Deref for MapSystemShared<M> {
  type Target = MapSystemFn<M>;

  fn deref(&self) -> &Self::Target {
    &*self.inner
  }
}

/// Shared wrapper around a `ReceiveTimeoutSchedulerFactory` implementation.
pub struct ReceiveTimeoutFactoryShared<M, R> {
  inner: ArcShared<dyn ReceiveTimeoutSchedulerFactory<M, R>>,
}

impl<M, R> ReceiveTimeoutFactoryShared<M, R>
where
  M: Element + 'static,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  /// Creates a new shared factory from a concrete factory value.
  pub fn new<F>(factory: F) -> Self
  where
    F: ReceiveTimeoutSchedulerFactory<M, R> + 'static, {
    Self {
      inner: ArcShared::from_arc(Arc::new(factory)),
    }
  }

  /// Wraps an existing shared factory.
  pub fn from_shared(inner: ArcShared<dyn ReceiveTimeoutSchedulerFactory<M, R>>) -> Self {
    Self { inner }
  }

  /// Consumes the wrapper and returns the underlying shared handle.
  pub fn into_shared(self) -> ArcShared<dyn ReceiveTimeoutSchedulerFactory<M, R>> {
    self.inner
  }
}

impl<M, R> Clone for ReceiveTimeoutFactoryShared<M, R> {
  fn clone(&self) -> Self {
    Self {
      inner: self.inner.clone(),
    }
  }
}

impl<M, R> Deref for ReceiveTimeoutFactoryShared<M, R> {
  type Target = dyn ReceiveTimeoutSchedulerFactory<M, R>;

  fn deref(&self) -> &Self::Target {
    &*self.inner
  }
}

/// Shared wrapper for failure event handlers.
pub struct FailureEventHandlerShared {
  inner: ArcShared<FailureEventHandlerFn>,
}

impl FailureEventHandlerShared {
  /// Creates a new shared handler from a closure.
  pub fn new<F>(handler: F) -> Self
  where
    F: Fn(&FailureInfo) + SharedBound + 'static, {
    Self {
      inner: ArcShared::from_arc(Arc::new(handler)),
    }
  }

  /// Wraps an existing shared handler reference.
  pub fn from_shared(inner: ArcShared<FailureEventHandlerFn>) -> Self {
    Self { inner }
  }

  /// Consumes the wrapper and returns the underlying shared handler.
  pub fn into_shared(self) -> ArcShared<FailureEventHandlerFn> {
    self.inner
  }
}

impl Clone for FailureEventHandlerShared {
  fn clone(&self) -> Self {
    Self {
      inner: self.inner.clone(),
    }
  }
}

impl Deref for FailureEventHandlerShared {
  type Target = FailureEventHandlerFn;

  fn deref(&self) -> &Self::Target {
    &*self.inner
  }
}

/// Shared wrapper for failure event listeners.
pub struct FailureEventListenerShared {
  inner: ArcShared<FailureEventListenerFn>,
}

impl FailureEventListenerShared {
  /// Creates a new shared listener from a closure.
  pub fn new<F>(listener: F) -> Self
  where
    F: Fn(FailureEvent) + SharedBound + 'static, {
    Self {
      inner: ArcShared::from_arc(Arc::new(listener)),
    }
  }

  /// Wraps an existing shared listener.
  pub fn from_shared(inner: ArcShared<FailureEventListenerFn>) -> Self {
    Self { inner }
  }

  /// Consumes the wrapper and returns the underlying shared listener.
  pub fn into_shared(self) -> ArcShared<FailureEventListenerFn> {
    self.inner
  }
}

impl Clone for FailureEventListenerShared {
  fn clone(&self) -> Self {
    Self {
      inner: self.inner.clone(),
    }
  }
}

impl Deref for FailureEventListenerShared {
  type Target = FailureEventListenerFn;

  fn deref(&self) -> &Self::Target {
    &*self.inner
  }
}
