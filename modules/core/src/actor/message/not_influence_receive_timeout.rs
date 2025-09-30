// Re-export NotInfluenceReceiveTimeout trait from core_types
pub use crate::actor::core_types::message_types::NotInfluenceReceiveTimeout;

use std::fmt::Debug;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct NotInfluenceReceiveTimeoutHandle(pub Arc<dyn NotInfluenceReceiveTimeout>);
