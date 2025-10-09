use core::fmt::Debug;

#[cfg(all(feature = "alloc", target_has_atomic = "ptr"))]
use alloc::sync::Arc;
#[cfg(feature = "alloc")]
use alloc::{boxed::Box, string::String};

/// Fundamental constraints for elements that can be stored in collections such as queues and stacks.
///
/// By requiring `Debug`, `Send`, `Sync`, and `'static`, this trait ensures element types
/// that can be safely handled in multithreaded environments.
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

#[cfg(all(feature = "alloc", target_has_atomic = "ptr"))]
impl<T> Element for Arc<T> where T: Debug + Send + Sync + 'static {}

impl<T> Element for Option<T> where T: Debug + Send + Sync + 'static {}
