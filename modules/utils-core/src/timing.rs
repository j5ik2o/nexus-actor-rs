//! タイマー関連の抽象をまとめたモジュール。
//!
//! `ReceiveTimeout` など時間起因の機能で共通利用するため、core から参照される最小限の API を再輸出する。

pub mod deadline_timer;

pub use deadline_timer::{
  DeadlineTimer, DeadlineTimerError, DeadlineTimerExpired, DeadlineTimerKey, DeadlineTimerKeyAllocator, TimerDeadline,
};
