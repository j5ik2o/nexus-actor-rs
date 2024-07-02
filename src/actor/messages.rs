use std::any::Any;
use std::fmt::Debug;
use std::future::Future;
use std::sync::Arc;

use futures::future::BoxFuture;

use crate::actor::actor::{ActorInnerError, PoisonPill, Stop};
use crate::actor::actor::pid::ExtendedPid;
use crate::actor::actor::restart_statistics::RestartStatistics;
use crate::actor::future::FutureError;
use crate::actor::message::{Message, MessageHandle};

#[derive(Debug, Clone)]
pub enum MailboxMessage {
  SuspendMailbox,
  ResumeMailbox,
}

impl Message for MailboxMessage {
  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }
}

#[derive(Debug, Clone)]
pub struct ReceiveTimeout {}

impl Message for ReceiveTimeout {
  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }
}

#[derive(Debug, Clone)]
pub struct IgnoreDeadLetterLogging {}

impl Message for IgnoreDeadLetterLogging {
  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }
}

#[derive(Debug, Clone)]
pub enum AutoReceiveMessage {
  Restarting(Restarting),
  Stopping(Stopping),
  Stopped(Stopped),
  PoisonPill(PoisonPill),
}

impl Message for AutoReceiveMessage {
  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }
}

impl AutoReceiveMessage {
  pub fn auto_receive_message(&self, pid: &ExtendedPid, message: MessageHandle) {}
}

pub trait NotInfluenceReceiveTimeout: Debug + Send + Sync + 'static {
  fn as_any(&self) -> &dyn Any;
  fn not_influence_receive_timeout(&self);
}

#[derive(Debug, Clone)]
pub struct NotInfluenceReceiveTimeoutHandle(pub Arc<dyn NotInfluenceReceiveTimeout>);

#[derive(Debug, Clone)]
pub enum SystemMessage {
  Restart(Restart),
  Started(Started),
  Stop(Stop),
}

impl Message for SystemMessage {
  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }
}

impl SystemMessage {
  fn as_any(&self) -> &dyn Any {
    self
  }

  fn system_message(&self) {}
}
#[derive(Debug, Clone)]
pub struct ReceiveTime;

#[derive(Debug, Clone)]
pub struct Restarting;

#[derive(Debug, Clone)]
pub struct Stopping;

#[derive(Debug, Clone)]
pub struct Stopped;

#[derive(Debug, Clone)]
pub struct Started;

#[derive(Debug, Clone)]
pub struct Restart {}

#[derive(Debug, Clone)]
pub struct Failure {
  pub who: ExtendedPid,
  pub reason: ActorInnerError,
  pub restart_stats: RestartStatistics,
  pub message: MessageHandle,
}

impl Message for Failure {
  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }
}

impl Failure {
  pub fn new(
    who: ExtendedPid,
    reason: ActorInnerError,
    restart_stats: RestartStatistics,
    message: MessageHandle,
  ) -> Self {
    Failure {
      who,
      reason,
      restart_stats,
      message,
    }
  }
}

#[derive(Clone)]
pub struct ContinuationHandler(pub Arc<dyn Fn() -> BoxFuture<'static, ()> + Send + Sync>);

impl ContinuationHandler {
  pub fn new<F, Fut>(f: F) -> Self
  where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static, {
    ContinuationHandler(Arc::new(move || Box::pin(f()) as BoxFuture<'static, ()>))
  }

  pub async fn run(&self) {
    (self.0)().await
  }
}

impl Debug for ContinuationHandler {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "ContinuationHandler")
  }
}

#[derive(Clone)]
pub(crate) struct Continuation {
  pub(crate) message: MessageHandle,
  pub(crate) f: ContinuationHandler,
}

impl Continuation {
  pub(crate) fn new<F, Fut>(message: MessageHandle, f: F) -> Self
  where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static, {
    Continuation {
      message,
      f: ContinuationHandler::new(move || Box::pin(f()) as BoxFuture<'static, ()>),
    }
  }
}

unsafe impl Send for Continuation {}
unsafe impl Sync for Continuation {}

impl Debug for Continuation {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Continuation").field("message", &self.message).finish()
  }
}

impl Message for Continuation {
  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }
}

#[derive(Clone)]
pub struct ContinuationFunc(
  Arc<dyn Fn(Option<MessageHandle>, Option<FutureError>) -> BoxFuture<'static, ()> + Send + Sync + 'static>,
);

impl ContinuationFunc {
  pub fn new<F>(f: F) -> Self
  where
    F: Fn(Option<MessageHandle>, Option<FutureError>) -> BoxFuture<'static, ()> + Send + Sync + 'static, {
    ContinuationFunc(Arc::new(f))
  }

  pub async fn run(&self, result: Option<MessageHandle>, error: Option<FutureError>) {
    (self.0)(result, error).await
  }
}

impl Debug for ContinuationFunc {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "ContinuationFunc")
  }
}
