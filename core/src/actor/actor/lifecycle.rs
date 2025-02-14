use crate::actor::context::Context;
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
  async fn pre_start(&mut self, ctx: &Context) -> Result<(), Box<dyn std::error::Error>>;
  async fn post_start(&mut self, ctx: &Context) -> Result<(), Box<dyn std::error::Error>>;
  async fn pre_restart(
    &mut self,
    ctx: &Context,
    reason: Option<Box<dyn std::error::Error>>,
  ) -> Result<(), Box<dyn std::error::Error>>;
  async fn post_restart(&mut self, ctx: &Context) -> Result<(), Box<dyn std::error::Error>>;
  async fn pre_stop(&mut self, ctx: &Context) -> Result<(), Box<dyn std::error::Error>>;
  async fn post_stop(&mut self, ctx: &Context) -> Result<(), Box<dyn std::error::Error>>;
  async fn terminated(&mut self, ctx: &Context, who: &crate::actor::pid::Pid)
    -> Result<(), Box<dyn std::error::Error>>;
}

impl<T: Send + Sync> Lifecycle for T {
  async fn pre_start(&mut self, _ctx: &Context) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
  }

  async fn post_start(&mut self, _ctx: &Context) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
  }

  async fn pre_restart(
    &mut self,
    _ctx: &Context,
    _reason: Option<Box<dyn std::error::Error>>,
  ) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
  }

  async fn post_restart(&mut self, _ctx: &Context) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
  }

  async fn pre_stop(&mut self, _ctx: &Context) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
  }

  async fn post_stop(&mut self, _ctx: &Context) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
  }

  async fn terminated(
    &mut self,
    _ctx: &Context,
    _who: &crate::actor::pid::Pid,
  ) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
  }
}
