use crate::actor::context::Context;
use crate::actor::pid::Pid;
use async_trait::async_trait;
use std::fmt::Debug;

#[derive(Debug, Clone, PartialEq)]
pub enum LifecycleEvent {
  PreStart,
  PostStart,
  PreRestart,
  PostRestart,
  PreStop,
  PostStop,
  Terminated,
}

#[async_trait]
pub trait Lifecycle: Send + Sync {
  async fn pre_start(&mut self, ctx: &dyn Context) -> Result<(), Box<dyn std::error::Error>>;
  async fn post_start(&mut self, ctx: &dyn Context) -> Result<(), Box<dyn std::error::Error>>;
  async fn pre_restart(
    &mut self,
    ctx: &dyn Context,
    reason: Option<Box<dyn std::error::Error>>,
  ) -> Result<(), Box<dyn std::error::Error>>;
  async fn post_restart(&mut self, ctx: &dyn Context) -> Result<(), Box<dyn std::error::Error>>;
  async fn pre_stop(&mut self, ctx: &dyn Context) -> Result<(), Box<dyn std::error::Error>>;
  async fn post_stop(&mut self, ctx: &dyn Context) -> Result<(), Box<dyn std::error::Error>>;
  async fn terminated(&mut self, ctx: &dyn Context, who: &Pid) -> Result<(), Box<dyn std::error::Error>>;
}

#[async_trait]
impl<T: Send + Sync + 'static> Lifecycle for T {
  async fn pre_start(&mut self, _ctx: &dyn Context) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
  }

  async fn post_start(&mut self, _ctx: &dyn Context) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
  }

  async fn pre_restart(
    &mut self,
    _ctx: &dyn Context,
    _reason: Option<Box<dyn std::error::Error>>,
  ) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
  }

  async fn post_restart(&mut self, _ctx: &dyn Context) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
  }

  async fn pre_stop(&mut self, _ctx: &dyn Context) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
  }

  async fn post_stop(&mut self, _ctx: &dyn Context) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
  }

  async fn terminated(&mut self, _ctx: &dyn Context, _who: &Pid) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
  }
}
