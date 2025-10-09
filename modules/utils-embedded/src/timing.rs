//! 組み込み向けタイマー実装をまとめるモジュール。
//!
//! `ManualDeadlineTimer` を公開し、ランタイムがソフトウェアカウンタで期限管理できるようにする。

mod manual_deadline_timer;

pub use manual_deadline_timer::ManualDeadlineTimer;
