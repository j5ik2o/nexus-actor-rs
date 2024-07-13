use std::env;
use std::sync::Mutex;

use once_cell::sync::Lazy;

use crate::log::log::LogLevel;
use crate::log::log_event::LogEvent;

#[derive(Clone, Debug)]
pub struct LogOptions {
  pub(crate) log_level: LogLevel,
  pub(crate) enable_caller: bool,
}

pub(crate) static DEVELOPMENT: Lazy<LogOptions> = Lazy::new(|| LogOptions {
  log_level: LogLevel::Debug,
  enable_caller: true,
});

pub(crate) static PRODUCTION: Lazy<LogOptions> = Lazy::new(|| LogOptions {
  log_level: LogLevel::Info,
  enable_caller: false,
});

pub(crate) static CURRENT: Lazy<Mutex<LogOptions>> = Lazy::new(|| {
  let env = env::var("PROTO_ACTOR_ENV").unwrap_or_default();
  let options = match env.as_str() {
    "dev" => DEVELOPMENT.clone(),
    "prod" | _ => PRODUCTION.clone(),
  };
  Mutex::new(options)
});

impl LogOptions {
  pub fn with(&self, opts: &[Box<dyn Fn(&mut LogOptions)>]) -> LogOptions {
    let mut cloned = self.clone();
    for opt in opts {
      opt(&mut cloned);
    }
    cloned
  }
}

pub fn with_event_subscriber(fn_: Option<fn(LogEvent)>) -> Box<dyn Fn(&mut LogOptions)> {
  Box::new(move |_opts: &mut LogOptions| {
    reset_event_subscriber(fn_);
  })
}

pub fn with_caller(enabled: bool) -> Box<dyn Fn(&mut LogOptions)> {
  Box::new(move |opts: &mut LogOptions| {
    opts.enable_caller = enabled;
  })
}

pub fn with_default_level(level: LogLevel) -> Box<dyn Fn(&mut LogOptions)> {
  Box::new(move |opts: &mut LogOptions| {
    opts.log_level = if level == LogLevel::Default {
      LogLevel::Info
    } else {
      level
    };
  })
}

pub fn set_options(opts: &[Box<dyn Fn(&mut LogOptions)>]) {
  let mut current = CURRENT.lock().unwrap();
  *current = current.with(opts);
}

// This function should be implemented based on your event system
fn reset_event_subscriber(_fn: Option<fn(LogEvent)>) {
  // Implementation here
}
