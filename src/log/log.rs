use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;

use num_enum::TryFromPrimitive;
use time::OffsetDateTime;

use crate::log::caller::CallerInfo;
use crate::log::event::Event;
use crate::log::field::Field;
use crate::log::options::CURRENT;
use crate::log::stream::{reset_event_stream, EventStream};
use crate::log::string_encoder::{reset_global_logger, reset_no_std_err_logs, reset_subscription};

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
  event_stream: Arc<EventStream>,
  level: Arc<AtomicI32>,
  prefix: String,
  context: Vec<Field>,
  enable_caller: bool,
}

impl Logger {
  pub fn new(event_stream: Arc<EventStream>, level: Level, prefix: &str, context: Vec<Field>) -> Self {
    let opts = CURRENT.lock().unwrap();
    let level = if level == Level::Default { opts.log_level } else { level };
    Logger {
      event_stream,
      level: Arc::new(AtomicI32::new(level as i32)),
      prefix: prefix.to_string(),
      context,
      enable_caller: opts.enable_caller,
    }
  }

  pub fn with_caller(mut self) -> Self {
    self.enable_caller = true;
    self
  }

  pub fn with(&self, fields: Vec<Field>) -> Self {
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

  pub fn level(&self) -> Level {
    let level = self.level.load(Ordering::Relaxed);
    let n: i32 = unsafe { std::mem::transmute(level) };
    Level::try_from(n).unwrap()
  }

  pub fn set_level(&self, level: Level) {
    self.level.store(level as i32, Ordering::Relaxed);
  }

  fn new_event(&self, msg: &str, level: Level, fields: Vec<Field>) -> Event {
    let mut ev = Event {
      time: OffsetDateTime::now_utc(),
      level,
      prefix: self.prefix.clone(),
      message: msg.to_string(),
      context: self.context.clone(),
      fields,
      caller: None,
    };
    if self.enable_caller {
      ev.caller = Some(CallerInfo::new(3));
    }
    ev
  }

  pub async fn debug(&self, msg: &str, fields: Vec<Field>) {
    if self.level() <= Level::Debug {
      self
        .event_stream
        .publish(self.new_event(msg, Level::Debug, fields))
        .await;
    }
  }

  pub async fn info(&self, msg: &str, fields: Vec<Field>) {
    if self.level() <= Level::Info {
      self
        .event_stream
        .publish(self.new_event(msg, Level::Info, fields))
        .await;
    }
  }

  pub async fn warn(&self, msg: &str, fields: Vec<Field>) {
    if self.level() <= Level::Warn {
      self
        .event_stream
        .publish(self.new_event(msg, Level::Warn, fields))
        .await;
    }
  }

  pub async fn error(&self, msg: &str, fields: Vec<Field>) {
    if self.level() <= Level::Error {
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

#[cfg(test)]
mod tests {
  use std::sync::RwLock;

  use crate::log::stream::{publish_to_stream, subscribe_stream, unsubscribe_stream, EventStream};

  use super::*;

  #[tokio::test]
  async fn test_logger_with() {
    let event_stream = Arc::new(EventStream::new());
    let base = Logger::new(event_stream, Level::Debug, "", vec![Field::string("first", "value")]);
    let l = base.with(vec![Field::string("second", "value")]);

    assert_eq!(
      vec![Field::string("first", "value"), Field::string("second", "value")],
      l.context
    );
  }

  #[tokio::test]
  async fn test_off_level_two_fields() {
    let event_stream = Arc::new(EventStream::new());
    let l = Logger::new(event_stream, Level::Min, "", vec![]);
    l.debug("foo", vec![Field::int("bar", 32), Field::bool("fum", false)])
      .await;
  }

  #[tokio::test]
  async fn test_off_level_only_context() {
    let event_stream = Arc::new(EventStream::new());
    let l = Logger::new(
      event_stream,
      Level::Min,
      "",
      vec![Field::int("bar", 32), Field::bool("fum", false)],
    );
    l.debug("foo", vec![]).await;
  }

  #[tokio::test]
  async fn test_debug_level_only_context_one_subscriber() {
    let event_stream = Arc::new(EventStream::new());
    let _s1 = subscribe_stream(&event_stream, |_: Event| {}).await;

    let l = Logger::new(
      event_stream,
      Level::Debug,
      "",
      vec![Field::int("bar", 32), Field::bool("fum", false)],
    );
    l.debug("foo", vec![]).await;

    unsubscribe_stream(&_s1).await;
  }

  #[tokio::test]
  async fn test_debug_level_only_context_multiple_subscribers() {
    let event_stream = Arc::new(EventStream::new());
    let _s1 = subscribe_stream(&event_stream, |_: Event| {}).await;
    let _s2 = subscribe_stream(&event_stream, |_: Event| {}).await;

    let l = Logger::new(
      event_stream,
      Level::Debug,
      "",
      vec![Field::int("bar", 32), Field::bool("fum", false)],
    );
    l.debug("foo", vec![]).await;

    unsubscribe_stream(&_s1).await;
    unsubscribe_stream(&_s2).await;
  }

  #[tokio::test]
  async fn test_subscribe_and_publish() {
    let event_stream = Arc::new(EventStream::new());
    let received = Arc::new(RwLock::new(Vec::new()));
    let received_clone = Arc::clone(&received);

    let sub = subscribe_stream(&event_stream, move |evt: Event| {
      received_clone.write().unwrap().push(evt.message.clone());
    })
    .await;

    publish_to_stream(&event_stream, Event::new(Level::Info, "Test message".to_string())).await;

    assert_eq!(received.read().unwrap().len(), 1);
    assert_eq!(received.read().unwrap()[0], "Test message");

    unsubscribe_stream(&sub).await;
  }

  #[tokio::test]
  async fn test_min_level_filtering() {
    let event_stream = Arc::new(EventStream::new());
    let received = Arc::new(RwLock::new(Vec::new()));
    let received_clone = Arc::clone(&received);

    let sub = subscribe_stream(&event_stream, move |evt: Event| {
      received_clone.write().unwrap().push(evt.message.clone());
    })
    .await
    .with_min_level(Level::Warn);

    publish_to_stream(&event_stream, Event::new(Level::Info, "Info message".to_string())).await;
    publish_to_stream(&event_stream, Event::new(Level::Warn, "Warn message".to_string())).await;
    publish_to_stream(&event_stream, Event::new(Level::Error, "Error message".to_string())).await;

    assert_eq!(received.read().unwrap().len(), 2);
    assert_eq!(received.read().unwrap()[0], "Warn message");
    assert_eq!(received.read().unwrap()[1], "Error message");

    unsubscribe_stream(&sub).await;
  }
}
