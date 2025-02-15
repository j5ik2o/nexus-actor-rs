use std::sync::Arc;
use tokio::sync::Mutex;

use crate::actor::{
  new_process_handle, ActorContext, DeadLetterProcess, EventStreamProcess, Message, MessageHandle, Pid, Process, Props,
  RootContext, SpawnError,
};

#[derive(Debug, Clone)]
pub struct ActorSystem {
  inner: Arc<Mutex<ActorSystemInner>>,
}

#[derive(Debug)]
struct ActorSystemInner {
  root_context: Option<RootContext>,
  dead_letter: Option<Box<dyn Process + Send + Sync>>,
  event_stream: Option<Box<dyn Process + Send + Sync>>,
}

impl ActorSystem {
  pub fn new() -> Self {
    let system = Self {
      inner: Arc::new(Mutex::new(ActorSystemInner {
        root_context: None,
        dead_letter: None,
        event_stream: None,
      })),
    };

    let system_clone = system.clone();
    tokio::spawn(async move {
      let mut inner = system_clone.inner.lock().await;
      inner.root_context = Some(RootContext::new(system_clone.clone()));
      inner.dead_letter = Some(new_process_handle(DeadLetterProcess::new(system_clone.clone())));
      inner.event_stream = Some(new_process_handle(EventStreamProcess::new(system_clone.clone())));
    });

    system
  }

  pub async fn spawn(&self, props: Props) -> Result<Pid, SpawnError> {
    let inner = self.inner.lock().await;
    if let Some(root) = &inner.root_context {
      root.spawn(props).await
    } else {
      Err(SpawnError::NoRootContext)
    }
  }

  pub async fn spawn_prefix(&self, props: Props, prefix: &str) -> Result<Pid, SpawnError> {
    let inner = self.inner.lock().await;
    if let Some(root) = &inner.root_context {
      root.spawn_prefix(props, prefix).await
    } else {
      Err(SpawnError::NoRootContext)
    }
  }

  pub async fn spawn_named(&self, props: Props, name: &str) -> Result<Pid, SpawnError> {
    let mut inner = self.inner.lock().await;
    if let Some(root) = &mut inner.root_context {
      root.spawn_named(props, name).await
    } else {
      Err(SpawnError::NoRootContext)
    }
  }

  pub async fn send_user_message(&self, pid: Pid, message: MessageHandle) {
    let inner = self.inner.lock().await;
    if let Some(dead_letter) = &inner.dead_letter {
      dead_letter.send_user_message(None, message).await;
    }
  }

  pub async fn request_future(&self, pid: Pid, message: MessageHandle) -> Result<ResponseHandle, ActorError> {
    let inner = self.inner.lock().await;
    if let Some(root) = &inner.root_context {
      root.request_future(pid, message).await
    } else {
      Err(ActorError::NoRootContext)
    }
  }

  pub async fn stop(&self, pid: &Pid) {
    let inner = self.inner.lock().await;
    if let Some(process) = inner.dead_letter.as_ref() {
      process.stop().await;
    }
  }

  pub async fn poison(&self, pid: &Pid) {
    let inner = self.inner.lock().await;
    if let Some(process) = inner.dead_letter.as_ref() {
      process.set_dead().await;
    }
  }

  pub async fn event_stream(&self) -> Option<Box<dyn Process + Send + Sync>> {
    let inner = self.inner.lock().await;
    inner.event_stream.clone()
  }
}
