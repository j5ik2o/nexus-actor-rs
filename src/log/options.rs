use std::env;
use std::sync::Mutex;

use once_cell::sync::Lazy;

use crate::log::log::Level;
use crate::log::log_event::LogEvent;

#[derive(Clone, Debug)]
pub struct Options {
  pub(crate) log_level: Level,
  pub(crate) enable_caller: bool,
}

pub static DEVELOPMENT: Lazy<Options> = Lazy::new(|| Options {
  log_level: Level::Debug,
  enable_caller: true,
});

pub static PRODUCTION: Lazy<Options> = Lazy::new(|| Options {
  log_level: Level::Info,
  enable_caller: false,
});

pub static CURRENT: Lazy<Mutex<Options>> = Lazy::new(|| {
  let env = env::var("PROTO_ACTOR_ENV").unwrap_or_default();
  let options = match env.as_str() {
    "dev" => DEVELOPMENT.clone(),
    "prod" | _ => PRODUCTION.clone(),
  };
  Mutex::new(options)
});

impl Options {
  pub fn with(&self, opts: &[Box<dyn Fn(&mut Options)>]) -> Options {
    let mut cloned = self.clone();
    for opt in opts {
      opt(&mut cloned);
    }
    cloned
  }
}

pub fn with_event_subscriber(fn_: Option<fn(LogEvent)>) -> Box<dyn Fn(&mut Options)> {
  Box::new(move |_opts: &mut Options| {
    // Assuming you have a function to reset the event subscriber
    reset_event_subscriber(fn_);
  })
}

pub fn with_caller(enabled: bool) -> Box<dyn Fn(&mut Options)> {
  Box::new(move |opts: &mut Options| {
    opts.enable_caller = enabled;
  })
}

pub fn with_default_level(level: Level) -> Box<dyn Fn(&mut Options)> {
  Box::new(move |opts: &mut Options| {
    opts.log_level = if level == Level::Default { Level::Info } else { level };
  })
}

pub fn set_options(opts: &[Box<dyn Fn(&mut Options)>]) {
  let mut current = CURRENT.lock().unwrap();
  *current = current.with(opts);
}

// This function should be implemented based on your event system
fn reset_event_subscriber(_fn: Option<fn(LogEvent)>) {
  // Implementation here
}
