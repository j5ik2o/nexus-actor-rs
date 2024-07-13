use crate::log::log::Level;
use crate::log::log_event_handler::LogEventHandler;
use crate::log::log_event_stream::LogEventStream;
use std::sync::atomic::{AtomicI32, AtomicUsize, Ordering};
use std::sync::{Arc, Weak};

#[derive(Debug, Clone)]
pub struct LogSubscription {
  pub(crate) event_stream: Weak<LogEventStream>,
  pub(crate) index: Arc<AtomicUsize>,
  pub(crate) func: LogEventHandler,
  pub(crate) min_level: Arc<AtomicI32>,
}

impl LogSubscription {
  pub fn with_min_level(self: &Arc<Self>, level: Level) -> Arc<Self> {
    self.min_level.store(level as i32, Ordering::Relaxed);
    Arc::clone(self)
  }
}
