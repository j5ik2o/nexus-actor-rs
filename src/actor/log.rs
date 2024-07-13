use once_cell::sync::Lazy;

use crate::log::event_stream::get_global_event_stream;
use crate::log::log::Logger;

pub static P_LOG: Lazy<Logger> = Lazy::new(|| {
  Logger::new(
    get_global_event_stream(),
    crate::log::log::Level::Debug,
    "[ACTOR]",
    vec![],
  )
});

pub fn set_log_level(level: crate::log::log::Level) {
  P_LOG.set_level(level);
}
