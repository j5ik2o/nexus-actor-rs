use once_cell::sync::Lazy;
use std::clone::Clone;

use crate::log::log::Logger;
use crate::log::log_event_stream::LOG_EVENT_STREAM;

pub static P_LOG: Lazy<Logger> =
  Lazy::new(|| Logger::new(LOG_EVENT_STREAM.clone(), crate::log::log::LogLevel::Debug, "[ACTOR]"));

pub fn set_log_level(level: crate::log::log::LogLevel) {
  P_LOG.set_level(level);
}
