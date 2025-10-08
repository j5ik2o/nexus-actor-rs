use super::sync::{tokio_core_runtime, TokioRuntime};

#[tokio::test(flavor = "current_thread")]
async fn test_tokio_runtime_provides_yielder() {
  let factory = TokioRuntime::default().core_runtime();
  let yielder = runtime.yielder().expect("yielder must be available");
  yielder.yield_now().await;
}

#[tokio::test(flavor = "current_thread")]
async fn test_tokio_core_runtime_yielder_completes() {
  let factory = tokio_core_runtime();
  let yielder = runtime.yielder().expect("yielder must be available");
  yielder.yield_now().await;
}
