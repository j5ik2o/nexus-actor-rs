use once_cell::sync::Lazy;
use std::clone::Clone;

use crate::log::Logger;
use crate::log::LOG_EVENT_STREAM;

pub static P_LOG: Lazy<Logger> =
  Lazy::new(|| Logger::new(LOG_EVENT_STREAM.clone(), crate::log::LogLevel::Debug, "[ACTOR]"));

pub fn set_log_level(level: crate::log::LogLevel) {
  P_LOG.set_level(level);
}
