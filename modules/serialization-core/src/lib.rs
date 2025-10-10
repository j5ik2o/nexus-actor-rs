#![cfg_attr(not(feature = "std"), no_std)]
#![deny(missing_docs)]

//! Core serialization abstractions shared by Nexus Actor modules.

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod error;
pub mod id;
#[cfg(feature = "serde-json")]
pub mod json;
pub mod message;
pub mod registry;
pub mod serializer;

pub use error::{DeserializationError, RegistryError, SerializationError};
pub use id::{SerializerId, TEST_ECHO_SERIALIZER_ID, USER_DEFINED_START};
#[cfg(feature = "serde-json")]
pub use json::{shared_json_serializer, SerdeJsonSerializer, SERDE_JSON_SERIALIZER_ID};
pub use message::{MessageHeader, SerializedMessage};
pub use registry::InMemorySerializerRegistry;
pub use serializer::Serializer;
