use core::fmt::Debug;

#[cfg(feature = "alloc")]
use alloc::{boxed::Box, string::String, sync::Arc};

pub trait Element: Debug + Send + Sync + 'static {}

macro_rules! impl_element_for_primitives {
  ($($ty:ty),* $(,)?) => {
    $(impl Element for $ty {})*
  };
}

impl_element_for_primitives!(i8, i16, i32, i64, isize);
impl_element_for_primitives!(u8, u16, u32, u64, usize);
impl_element_for_primitives!(f32, f64, bool, char);

#[cfg(feature = "alloc")]
impl Element for String {}

#[cfg(feature = "alloc")]
impl<T> Element for Box<T> where T: Debug + Send + Sync + 'static {}

#[cfg(feature = "alloc")]
impl<T> Element for Arc<T> where T: Debug + Send + Sync + 'static {}

impl<T> Element for Option<T> where T: Debug + Send + Sync + 'static {}
