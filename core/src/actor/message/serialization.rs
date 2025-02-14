use super::Message;
use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum SerializationError {
    SerializationFailed(String),
    DeserializationFailed(String),
    UnsupportedFormat,
}

impl fmt::Display for SerializationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SerializationError::SerializationFailed(msg) => write!(f, "Serialization failed: {}", msg),
            SerializationError::DeserializationFailed(msg) => write!(f, "Deserialization failed: {}", msg),
            SerializationError::UnsupportedFormat => write!(f, "Unsupported serialization format"),
        }
    }
}

impl Error for SerializationError {}

pub trait MessageSerializer: Send + Sync {
    fn serialize(&self, message: &dyn Message) -> Result<Vec<u8>, SerializationError>;
    fn deserialize(&self, bytes: &[u8]) -> Result<Box<dyn Message>, SerializationError>;
    fn get_format_id(&self) -> u32;
}

pub trait SerializableMessage: Message {
    fn to_bytes(&self) -> Result<Vec<u8>, SerializationError>;
    fn from_bytes(bytes: &[u8]) -> Result<Self, SerializationError> where Self: Sized;
}
