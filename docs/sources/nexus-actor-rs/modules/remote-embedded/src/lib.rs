#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(any(feature = "std", feature = "alloc")))]
compile_error!("nexus-remote-embedded-rs requires either the `std` or `alloc` feature.");

#[cfg(any(feature = "std", feature = "alloc"))]
extern crate alloc;

#[cfg(any(feature = "std", feature = "alloc"))]
use alloc::boxed::Box;

use core::sync::atomic::{AtomicBool, Ordering};

pub use nexus_remote_core_rs::core_api::{
  BlockListStore, BoxFuture, EndpointHandle, MetricsSink, RemoteRuntime, RemoteRuntimeConfig, RemoteTransport,
  SerializerRegistry, TransportEndpoint, TransportError, TransportErrorKind, TransportListener,
};

/// Minimal in-memory transport for embedded experimentation.
#[derive(Debug, Default, Clone, Copy)]
pub struct LoopbackTransport;

impl LoopbackTransport {
  #[inline]
  pub const fn new() -> Self {
    Self
  }
}

/// Handle returned by [`LoopbackTransport`].
#[derive(Debug)]
pub struct LoopbackHandle {
  endpoint: TransportEndpoint,
  closed: AtomicBool,
}

impl LoopbackHandle {
  #[inline]
  fn new(endpoint: TransportEndpoint) -> Self {
    Self {
      endpoint,
      closed: AtomicBool::new(false),
    }
  }

  /// Returns the endpoint descriptor associated with this handle.
  #[inline]
  pub fn endpoint(&self) -> &TransportEndpoint {
    &self.endpoint
  }

  /// Reports whether `close` has been invoked.
  #[inline]
  pub fn is_closed(&self) -> bool {
    self.closed.load(Ordering::SeqCst)
  }
}

impl EndpointHandle for LoopbackHandle {
  #[inline]
  fn close(&self) {
    self.closed.store(true, Ordering::SeqCst);
  }
}

impl RemoteTransport for LoopbackTransport {
  type Handle = LoopbackHandle;

  #[inline]
  fn connect(&self, endpoint: &TransportEndpoint) -> BoxFuture<'_, Result<Self::Handle, TransportError>> {
    let endpoint = endpoint.clone();
    Box::pin(async move { Ok(LoopbackHandle::new(endpoint)) })
  }

  #[inline]
  fn serve(&self, _listener: TransportListener) -> BoxFuture<'_, Result<(), TransportError>> {
    Box::pin(async { Ok(()) })
  }
}

#[cfg(all(test, feature = "std"))]
mod tests {
  use super::{
    BlockListStore, EndpointHandle, LoopbackTransport, MetricsSink, RemoteRuntime, RemoteRuntimeConfig,
    RemoteTransport, SerializerRegistry, TransportEndpoint, TransportListener,
  };
  use futures::executor::block_on;
  use nexus_actor_core_rs::{
    CoreRuntime, CoreRuntimeConfig, CoreScheduledHandle, CoreScheduledHandleRef, CoreScheduledTask, CoreScheduler,
    CoreSpawnError, Timer,
  };
  use std::{future::Future, pin::Pin, sync::Arc, time::Duration};

  #[derive(Default)]
  struct DummyTimer;

  impl Timer for DummyTimer {
    fn sleep(&self, _duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
      Box::pin(async {})
    }
  }

  #[derive(Default)]
  struct DummyScheduledHandle;

  impl CoreScheduledHandle for DummyScheduledHandle {
    fn cancel(&self) {}
  }

  #[derive(Default)]
  struct DummyScheduler;

  impl CoreScheduler for DummyScheduler {
    fn schedule_once(&self, _delay: Duration, task: CoreScheduledTask) -> CoreScheduledHandleRef {
      let _ = task();
      Arc::new(DummyScheduledHandle::default())
    }

    fn schedule_repeated(
      &self,
      _initial_delay: Duration,
      _interval: Duration,
      task: CoreScheduledTask,
    ) -> CoreScheduledHandleRef {
      let _ = task();
      Arc::new(DummyScheduledHandle::default())
    }
  }

  fn make_core_runtime() -> CoreRuntime {
    CoreRuntime::from_config(CoreRuntimeConfig::new(
      Arc::new(DummyTimer::default()),
      Arc::new(DummyScheduler::default()),
    ))
  }

  struct DummySerializer;

  impl SerializerRegistry for DummySerializer {}

  struct DummyBlockList;

  impl BlockListStore for DummyBlockList {
    fn is_blocked(&self, _system: &str) -> bool {
      false
    }
  }

  struct DummyMetrics;

  impl MetricsSink for DummyMetrics {}

  #[test]
  fn loopback_connects_and_closes() {
    let transport = LoopbackTransport::new();
    let endpoint = TransportEndpoint::new("loopback://peer".to_string());
    let handle = block_on(transport.connect(&endpoint)).expect("connect succeeds");
    assert_eq!(handle.endpoint().uri, endpoint.uri);
    assert!(!handle.is_closed());
    handle.close();
    assert!(handle.is_closed());
  }

  #[test]
  fn loopback_serve_is_noop() {
    let transport = LoopbackTransport::new();
    let listener = TransportListener::new("loopback://listen".to_string());
    block_on(transport.serve(listener)).expect("serve succeeds");
  }

  #[test]
  fn remote_runtime_configures_components() {
    let transport = Arc::new(LoopbackTransport::new());
    let serializer: Arc<dyn SerializerRegistry> = Arc::new(DummySerializer);
    let block_list: Arc<dyn BlockListStore> = Arc::new(DummyBlockList);
    let metrics: Arc<dyn MetricsSink> = Arc::new(DummyMetrics);

    let factory = RemoteRuntime::new(
      RemoteRuntimeConfig::new(make_core_runtime(), transport.clone())
        .with_serializer(serializer.clone())
        .with_block_list(block_list.clone())
        .with_metrics(metrics.clone()),
    );

    assert!(Arc::ptr_eq(&runtime.transport(), &transport));
    assert!(runtime.serializer().is_some());
    assert!(runtime.block_list().is_some());
    assert!(runtime.metrics().is_some());

    let spawn_result = runtime.spawn(async {});
    assert!(matches!(spawn_result, Err(CoreSpawnError::ExecutorUnavailable)));
  }
}
