#[cfg(test)]
mod tests;

use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct SleepContainer(Arc<RwLock<Pin<Box<tokio::time::Sleep>>>>);
impl SleepContainer {
  pub fn from_sleep(sleep: tokio::time::Sleep) -> Self {
    SleepContainer(Arc::new(RwLock::new(Box::pin(sleep))))
  }

  pub fn new(duration: Duration) -> Self {
    Self::from_sleep(tokio::time::sleep(duration))
  }

  pub fn from_underlying(underlying: Arc<RwLock<Pin<Box<tokio::time::Sleep>>>>) -> Self {
    Self(underlying)
  }

  pub async fn init(&mut self, instant: tokio::time::Instant) {
    let mut timer = self.0.write().await;
    *timer = Box::pin(tokio::time::sleep_until(instant));
  }

  pub async fn reset(&mut self, instant: tokio::time::Instant) {
    let mut sleep = self.0.write().await;
    sleep.as_mut().reset(instant);
  }

  pub async fn stop(&mut self) {
    let mut sleep = self.0.write().await;
    sleep.as_mut().reset(tokio::time::Instant::now());
  }

  pub async fn wait(&self) {
    let mut sleep = self.0.write().await;
    sleep.as_mut().await;
  }
}

#[derive(Debug, Clone)]
pub struct ReceiveTimeoutTimer(SleepContainer);

impl ReceiveTimeoutTimer {
  pub fn new(duration: Duration) -> Self {
    ReceiveTimeoutTimer(SleepContainer::new(duration))
  }

  pub fn from_sleep(sleep: tokio::time::Sleep) -> Self {
    ReceiveTimeoutTimer(SleepContainer::from_sleep(sleep))
  }

  pub fn from_underlying(underlying: Arc<RwLock<Pin<Box<tokio::time::Sleep>>>>) -> Self {
    ReceiveTimeoutTimer(SleepContainer::from_underlying(underlying))
  }

  pub async fn reset(&mut self, instant: tokio::time::Instant) {
    self.0.reset(instant).await;
  }

  pub async fn init(&mut self, instant: tokio::time::Instant) {
    self.0.init(instant).await;
  }

  pub async fn stop(&mut self) {
    self.0.stop().await;
  }

  pub async fn wait(&self) {
    self.0.wait().await;
  }
}
