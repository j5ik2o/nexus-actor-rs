use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;

use num_enum::TryFromPrimitive;
use time::OffsetDateTime;

use crate::log::log_caller::LogCallerInfo;
use crate::log::log_event::LogEvent;
use crate::log::log_event_stream::{reset_event_stream, LogEventStream};
use crate::log::log_field::LogField;
use crate::log::log_options::CURRENT;
use crate::log::log_string_encoder::{reset_global_logger, reset_no_std_err_logs, reset_subscription};

#[derive(Debug, Clone, Copy, PartialOrd, PartialEq, Eq, TryFromPrimitive)]
#[repr(i32)]
pub enum Level {
  Min = 0,
  Debug = 1,
  Info = 2,
  Warn = 3,
  Error = 4,
  Off = 5,
  Default = 6,
}

impl std::fmt::Display for Level {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let str = match self {
      Level::Min => "-    ",
      Level::Debug => "DEBUG",
      Level::Info => "INFO ",
      Level::Warn => "WARN ",
      Level::Error => "ERROR",
      Level::Off => "-    ",
      Level::Default => "INFO ",
    };
    write!(f, "{}", str)
  }
}

pub struct Logger {
  event_stream: Arc<LogEventStream>,
  level: Arc<AtomicI32>,
  prefix: String,
  context: Vec<LogField>,
  enable_caller: bool,
}

impl Logger {
  pub fn new(
    event_stream: Arc<LogEventStream>,
    level: Level,
    prefix: &str,
    context: impl IntoIterator<Item = LogField>,
  ) -> Self {
    let opts = CURRENT.lock().unwrap();
    let level = if level == Level::Default { opts.log_level } else { level };
    Logger {
      event_stream,
      level: Arc::new(AtomicI32::new(level as i32)),
      prefix: prefix.to_string(),
      context: context.into_iter().collect(),
      enable_caller: opts.enable_caller,
    }
  }

  pub fn with_caller(mut self) -> Self {
    self.enable_caller = true;
    self
  }

  pub fn with(&self, fields: impl IntoIterator<Item = LogField>) -> Self {
    let fields = fields.into_iter().collect::<Vec<_>>();
    let mut ctx = self.context.clone();
    ctx.extend(fields);
    Logger {
      event_stream: self.event_stream.clone(),
      level: self.level.clone(),
      prefix: self.prefix.clone(),
      context: ctx,
      enable_caller: self.enable_caller,
    }
  }

  pub fn get_level(&self) -> Level {
    let level = self.level.load(Ordering::Relaxed);
    let n: i32 = unsafe { std::mem::transmute(level) };
    Level::try_from(n).unwrap()
  }

  pub fn set_level(&self, level: Level) {
    self.level.store(level as i32, Ordering::Relaxed);
  }

  pub fn get_context(&self) -> Vec<LogField> {
    self.context.clone()
  }

  fn new_event(&self, msg: &str, level: Level, fields: impl IntoIterator<Item = LogField>) -> LogEvent {
    let mut ev = LogEvent {
      time: OffsetDateTime::now_utc(),
      level,
      prefix: self.prefix.clone(),
      message: msg.to_string(),
      context: self.context.clone(),
      fields: fields.into_iter().collect(),
      caller: None,
    };
    if self.enable_caller {
      ev.caller = Some(LogCallerInfo::new(3));
    }
    ev
  }

  pub async fn debug(&self, msg: &str, fields: impl IntoIterator<Item = LogField>) {
    if self.get_level() <= Level::Debug {
      self
        .event_stream
        .publish(self.new_event(msg, Level::Debug, fields))
        .await;
    }
  }

  pub async fn info(&self, msg: &str, fields: impl IntoIterator<Item = LogField>) {
    if self.get_level() <= Level::Info {
      self
        .event_stream
        .publish(self.new_event(msg, Level::Info, fields))
        .await;
    }
  }

  pub async fn warn(&self, msg: &str, fields: impl IntoIterator<Item = LogField>) {
    if self.get_level() <= Level::Warn {
      self
        .event_stream
        .publish(self.new_event(msg, Level::Warn, fields))
        .await;
    }
  }

  pub async fn error(&self, msg: &str, fields: impl IntoIterator<Item = LogField>) {
    if self.get_level() <= Level::Error {
      self
        .event_stream
        .publish(self.new_event(msg, Level::Error, fields))
        .await;
    }
  }
}

pub async fn reset_logger() {
  reset_event_stream().await;
  reset_no_std_err_logs().await;
  reset_global_logger().await;
  reset_subscription().await;
}
