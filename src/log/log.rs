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
pub enum LogLevel {
  Min = 0,
  Debug = 1,
  Info = 2,
  Warn = 3,
  Error = 4,
  Off = 5,
  Default = 6,
}

impl std::fmt::Display for LogLevel {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let str = match self {
      LogLevel::Min => "-    ",
      LogLevel::Debug => "DEBUG",
      LogLevel::Info => "INFO ",
      LogLevel::Warn => "WARN ",
      LogLevel::Error => "ERROR",
      LogLevel::Off => "-    ",
      LogLevel::Default => "INFO ",
    };
    write!(f, "{}", str)
  }
}

#[derive(Debug, Clone)]
pub struct Logger {
  event_stream: Arc<LogEventStream>,
  level: Arc<AtomicI32>,
  prefix: String,
  context: Vec<LogField>,
  enable_caller: bool,
}

impl Logger {
  pub fn new(log_event_stream: Arc<LogEventStream>, level: LogLevel, prefix: &str) -> Self {
    let opts = CURRENT.lock().unwrap();
    let level = if level == LogLevel::Default {
      opts.log_level
    } else {
      level
    };
    Logger {
      event_stream: log_event_stream,
      level: Arc::new(AtomicI32::new(level as i32)),
      prefix: prefix.to_string(),
      context: vec![],
      enable_caller: opts.enable_caller,
    }
  }

  pub fn with_caller(mut self) -> Self {
    self.enable_caller = true;
    self
  }

  pub fn with_fields(&self, fields: impl IntoIterator<Item = LogField>) -> Self {
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

  pub fn get_level(&self) -> LogLevel {
    let level = self.level.load(Ordering::Relaxed);
    let n: i32 = unsafe { std::mem::transmute(level) };
    LogLevel::try_from(n).unwrap()
  }

  pub fn set_level(&self, level: LogLevel) {
    self.level.store(level as i32, Ordering::Relaxed);
  }

  pub fn get_context(&self) -> Vec<LogField> {
    self.context.clone()
  }

  fn new_event(&self, msg: &str, level: LogLevel, fields: impl IntoIterator<Item = LogField>) -> LogEvent {
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

  pub async fn debug(&self, msg: &str) {
    if self.get_level() <= LogLevel::Debug {
      self
        .event_stream
        .publish(self.new_event(msg, LogLevel::Debug, []))
        .await;
    }
  }

  pub async fn debug_with_fields(&self, msg: &str, fields: impl IntoIterator<Item = LogField>) {
    if self.get_level() <= LogLevel::Debug {
      self
        .event_stream
        .publish(self.new_event(msg, LogLevel::Debug, fields))
        .await;
    }
  }

  pub async fn info(&self, msg: &str) {
    if self.get_level() <= LogLevel::Info {
      self.event_stream.publish(self.new_event(msg, LogLevel::Info, [])).await;
    }
  }

  pub async fn info_with_fields(&self, msg: &str, fields: impl IntoIterator<Item = LogField>) {
    if self.get_level() <= LogLevel::Info {
      self
        .event_stream
        .publish(self.new_event(msg, LogLevel::Info, fields))
        .await;
    }
  }

  pub async fn warn(&self, msg: &str) {
    if self.get_level() <= LogLevel::Warn {
      self.event_stream.publish(self.new_event(msg, LogLevel::Warn, [])).await;
    }
  }

  pub async fn warn_with_fields(&self, msg: &str, fields: impl IntoIterator<Item = LogField>) {
    if self.get_level() <= LogLevel::Warn {
      self
        .event_stream
        .publish(self.new_event(msg, LogLevel::Warn, fields))
        .await;
    }
  }

  pub async fn error(&self, msg: &str) {
    if self.get_level() <= LogLevel::Error {
      self
        .event_stream
        .publish(self.new_event(msg, LogLevel::Error, []))
        .await;
    }
  }

  pub async fn error_with_fields(&self, msg: &str, fields: impl IntoIterator<Item = LogField>) {
    if self.get_level() <= LogLevel::Error {
      self
        .event_stream
        .publish(self.new_event(msg, LogLevel::Error, fields))
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
