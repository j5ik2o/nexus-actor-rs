//! std ランタイムで利用するタイマー実装の束ね。
//!
//! 現状は `TokioDeadlineTimer` のみを公開し、`ReceiveTimeout` などの高レベル API から利用する。

mod tokio_deadline_timer;

pub use tokio_deadline_timer::TokioDeadlineTimer;
