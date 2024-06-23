use std::fmt::Debug;
use std::sync::Arc;

pub trait Element: Debug + Send + Sync + 'static {}

impl Element for i8 {}

impl Element for i16 {}

impl Element for i32 {}

impl Element for i64 {}

impl Element for u8 {}

impl Element for u16 {}

impl Element for u32 {}

impl Element for u64 {}

impl Element for usize {}

impl Element for f32 {}

impl Element for f64 {}

impl Element for String {}

impl<T: Debug + Send + Sync + 'static> Element for Box<T> {}

impl<T: Debug + Send + Sync + 'static> Element for Arc<T> {}

impl<T: Debug + Send + Sync + 'static> Element for Option<T> {}
