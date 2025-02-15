//! Message trait implementation.

use std::any::Any;
use std::fmt::Debug;

pub trait Message: Debug + Send + Sync + 'static + Clone + PartialEq {
    fn as_any(&self) -> &dyn Any;
    fn message_type(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
    fn eq_message(&self, other: &dyn Message) -> bool {
        if let Some(other) = other.as_any().downcast_ref::<Self>() {
            self == other
        } else {
            false
        }
    }
}

// Remove blanket implementation to avoid conflicts with derive macro
