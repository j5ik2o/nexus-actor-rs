use std::any::Any;
use std::fmt::Debug;

pub trait Message: Debug + Send + Sync + 'static {
    fn as_any(&self) -> &dyn Any;
    fn message_type(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

// Remove blanket implementation to avoid conflicts
