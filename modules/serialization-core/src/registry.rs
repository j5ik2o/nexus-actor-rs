//! In-memory serializer registry implementation.

#[cfg(feature = "alloc")]
use alloc::collections::BTreeMap;
#[cfg(all(feature = "alloc", not(target_has_atomic = "ptr")))]
use alloc::rc::Rc as SharedArc;
#[cfg(all(feature = "alloc", target_has_atomic = "ptr"))]
use alloc::sync::Arc as SharedArc;

#[cfg(feature = "alloc")]
use crate::error::RegistryError;
#[cfg(feature = "alloc")]
use crate::id::SerializerId;
#[cfg(feature = "alloc")]
use crate::serializer::Serializer;
#[cfg(feature = "alloc")]
use nexus_utils_core_rs::sync::ArcShared;
#[cfg(feature = "alloc")]
use spin::RwLock;

/// Default registry backed by a thread-safe map of serializer identifiers.
#[cfg(feature = "alloc")]
#[derive(Clone)]
pub struct InMemorySerializerRegistry {
  inner: ArcShared<RwLock<BTreeMap<SerializerId, ArcShared<dyn Serializer>>>>,
}

#[cfg(feature = "alloc")]
impl InMemorySerializerRegistry {
  /// Creates a new, empty registry.
  #[must_use]
  pub fn new() -> Self {
    Self {
      inner: ArcShared::new(RwLock::new(BTreeMap::new())),
    }
  }

  fn insert_serializer<S>(&self, serializer: ArcShared<S>) -> Result<(), RegistryError>
  where
    S: Serializer + 'static, {
    let serializer_id = serializer.serializer_id();
    let mut guard = self.inner.write();
    if guard.contains_key(&serializer_id) {
      return Err(RegistryError::DuplicateEntry(serializer_id));
    }
    let typed_arc: SharedArc<S> = serializer.into_arc();
    let trait_arc: SharedArc<dyn Serializer> = typed_arc;
    guard.insert(serializer_id, ArcShared::from_arc(trait_arc));
    Ok(())
  }

  /// Registers a serializer implementation.
  pub fn register<S>(&self, serializer: ArcShared<S>) -> Result<(), RegistryError>
  where
    S: Serializer + 'static, {
    self.insert_serializer(serializer)
  }

  /// Registers a trait-object serializer.
  pub fn register_trait(&self, serializer: ArcShared<dyn Serializer>) -> Result<(), RegistryError> {
    let serializer_id = serializer.serializer_id();
    let mut guard = self.inner.write();
    if guard.contains_key(&serializer_id) {
      return Err(RegistryError::DuplicateEntry(serializer_id));
    }
    guard.insert(serializer_id, serializer);
    Ok(())
  }

  /// Retrieves the serializer associated with the provided identifier.
  #[must_use]
  pub fn get(&self, serializer_id: SerializerId) -> Option<ArcShared<dyn Serializer>> {
    self.inner.read().get(&serializer_id).cloned()
  }
}

#[cfg(feature = "alloc")]
impl Default for InMemorySerializerRegistry {
  fn default() -> Self {
    Self::new()
  }
}

#[cfg(all(feature = "alloc", test))]
mod tests {
  use super::*;
  use crate::error::{DeserializationError, SerializationError};
  use crate::id::{SerializerId, TEST_ECHO_SERIALIZER_ID};
  use crate::message::SerializedMessage;
  use alloc::vec::Vec;

  #[derive(Debug)]
  struct EchoSerializer;

  impl Serializer for EchoSerializer {
    fn serializer_id(&self) -> SerializerId {
      TEST_ECHO_SERIALIZER_ID
    }

    fn content_type(&self) -> &str {
      "application/octet-stream"
    }

    fn serialize_with_type_name_opt(
      &self,
      payload: &[u8],
      type_name: Option<&str>,
    ) -> Result<SerializedMessage, SerializationError> {
      let mut message = SerializedMessage::new(self.serializer_id(), payload.to_vec());
      if let Some(name) = type_name {
        message = message.with_type_name(name);
      }
      Ok(message)
    }

    fn deserialize(&self, message: &SerializedMessage) -> Result<Vec<u8>, DeserializationError> {
      Ok(message.payload.clone())
    }
  }

  #[test]
  fn registers_and_resolves_serializer() {
    let registry = InMemorySerializerRegistry::new();
    let serializer = ArcShared::new(EchoSerializer);
    registry.register(serializer.clone()).expect("register");

    let resolved = registry.get(TEST_ECHO_SERIALIZER_ID).expect("resolve");
    assert_eq!(resolved.serializer_id(), TEST_ECHO_SERIALIZER_ID);

    let serialized = resolved
      .serialize_with_type_name(b"ping", "Example")
      .expect("serialize");
    let payload = resolved.deserialize(&serialized).expect("deserialize");
    assert_eq!(payload, b"ping");
  }

  #[test]
  fn rejects_duplicate_ids() {
    let registry = InMemorySerializerRegistry::new();
    let first = ArcShared::new(EchoSerializer);
    let second = ArcShared::new(EchoSerializer);

    registry.register(first).expect("register first");
    let err = registry.register(second).expect_err("duplicate");
    assert!(matches!(err, RegistryError::DuplicateEntry(id) if id == TEST_ECHO_SERIALIZER_ID));
  }
}
