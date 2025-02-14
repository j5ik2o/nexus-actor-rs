use std::any::Any;
use std::fmt::Debug;

pub trait Event: Any + Debug + Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn event_type(&self) -> &'static str;
}

#[derive(Debug)]
pub struct ClusterEvent {
    pub event_type: &'static str,
    pub data: Box<dyn Any + Send + Sync>,
}

impl Event for ClusterEvent {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn event_type(&self) -> &'static str {
        self.event_type
    }
}

#[derive(Debug)]
pub struct RemoteEvent {
    pub event_type: &'static str,
    pub data: Box<dyn Any + Send + Sync>,
}

impl Event for RemoteEvent {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn event_type(&self) -> &'static str {
        self.event_type
    }
}
