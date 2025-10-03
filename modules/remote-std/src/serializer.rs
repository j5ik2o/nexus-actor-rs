use dashmap::DashMap;
use nexus_actor_core_rs::actor::core_types::serialized::CoreSerializationError;
use nexus_actor_std_rs::actor::message::Message;
use once_cell::sync::Lazy;
use prost::Message as ProstMessage;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::fmt::{Display, Formatter};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::marker::PhantomData;
use std::sync::Arc;

#[cfg(test)]
mod tests;

pub type SerializerError = CoreSerializationError;

pub use nexus_actor_core_rs::actor::core_types::serialized::CoreRootSerializable as RootSerializable;
pub use nexus_actor_core_rs::actor::core_types::serialized::CoreRootSerialized as RootSerialized;

pub trait Serializer<T>: Send + Sync {
  fn serialize(&self, msg: &T) -> Result<Vec<u8>, SerializerError>;
  fn deserialize(&self, bytes: &[u8]) -> Result<T, SerializerError>;
  fn get_type_name(&self) -> String;
}

pub trait SerializerAny: Send + Sync {
  fn serialize_any(&self, msg: &dyn Any) -> Result<Vec<u8>, SerializerError>;
  fn deserialize_any(&self, bytes: &[u8]) -> Result<Arc<dyn Any + Send + Sync>, SerializerError>;
  fn deserialize_message(&self, bytes: &[u8]) -> Result<Arc<dyn Message>, SerializerError>;
  fn type_name(&self) -> String;
}

struct ProtoSerializer<T: ProstMessage> {
  _phantom: PhantomData<T>,
}

impl<T: ProstMessage + Default> Default for ProtoSerializer<T> {
  fn default() -> Self {
    Self { _phantom: PhantomData }
  }
}

impl<T: Message + ProstMessage + Default + 'static> SerializerAny for ProtoSerializer<T> {
  fn serialize_any(&self, msg: &dyn Any) -> Result<Vec<u8>, SerializerError> {
    msg
      .downcast_ref::<T>()
      .ok_or_else(|| SerializerError::serialization("Invalid type"))
      .and_then(|m| self.serialize(m))
  }

  fn deserialize_any(&self, bytes: &[u8]) -> Result<Arc<dyn Any + Send + Sync>, SerializerError> {
    self
      .deserialize(bytes)
      .map(|m| Arc::new(m) as Arc<dyn Any + Send + Sync>)
      .map_err(|e| SerializerError::deserialization(e.to_string()))
  }

  fn deserialize_message(&self, bytes: &[u8]) -> Result<Arc<dyn Message>, SerializerError> {
    self
      .deserialize(bytes)
      .map(|m| Arc::new(m) as Arc<dyn Message>)
      .map_err(|e| SerializerError::deserialization(e.to_string()))
  }

  fn type_name(&self) -> String {
    std::any::type_name::<T>().to_string()
  }
}

impl<T: Message + ProstMessage + Default> Serializer<T> for ProtoSerializer<T> {
  fn serialize(&self, msg: &T) -> Result<Vec<u8>, SerializerError> {
    Ok(msg.encode_to_vec())
  }

  fn deserialize(&self, bytes: &[u8]) -> Result<T, SerializerError> {
    T::decode(bytes).map_err(|e| SerializerError::deserialization(e.to_string()))
  }

  fn get_type_name(&self) -> String {
    std::any::type_name::<T>().to_string()
  }
}

struct JsonSerializer<T> {
  _phantom: PhantomData<T>,
}

impl<T: Serialize + for<'de> Deserialize<'de> + Send + Sync> Default for JsonSerializer<T> {
  fn default() -> Self {
    Self { _phantom: PhantomData }
  }
}

impl<T: Message + Serialize + for<'de> Deserialize<'de> + Send + Sync + 'static> SerializerAny for JsonSerializer<T> {
  fn serialize_any(&self, msg: &dyn Any) -> Result<Vec<u8>, SerializerError> {
    msg
      .downcast_ref::<T>()
      .ok_or_else(|| SerializerError::serialization("Invalid type"))
      .and_then(|m| self.serialize(m))
  }

  fn deserialize_any(&self, bytes: &[u8]) -> Result<Arc<dyn Any + Send + Sync>, SerializerError> {
    self
      .deserialize(bytes)
      .map(|m| Arc::new(m) as Arc<dyn Any + Send + Sync>)
      .map_err(|e| SerializerError::deserialization(e.to_string()))
  }

  fn deserialize_message(&self, bytes: &[u8]) -> Result<Arc<dyn Message>, SerializerError> {
    self
      .deserialize(bytes)
      .map(|m| Arc::new(m) as Arc<dyn Message>)
      .map_err(|e| SerializerError::deserialization(e.to_string()))
  }

  fn type_name(&self) -> String {
    std::any::type_name::<T>().to_string()
  }
}

impl<T: Serialize + for<'de> Deserialize<'de> + Send + Sync> Serializer<T> for JsonSerializer<T> {
  fn serialize(&self, msg: &T) -> Result<Vec<u8>, SerializerError> {
    serde_json::to_vec(&msg).map_err(|e| SerializerError::serialization(e.to_string()))
  }

  fn deserialize(&self, bytes: &[u8]) -> Result<T, SerializerError> {
    serde_json::from_slice(bytes).map_err(|e| SerializerError::deserialization(e.to_string()))
  }

  fn get_type_name(&self) -> String {
    std::any::type_name::<T>().to_string()
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum SerializerId {
  None = 0,
  Proto = 1,
  Json = 2,
  Custom(u32),
}

impl SerializerId {
  pub fn of_proto() -> Self {
    SerializerId::Proto
  }

  pub fn of_json() -> Self {
    SerializerId::Json
  }

  pub fn of_custom(value: u32) -> Self {
    if value <= 100 {
      panic!("Custom serializer id must be greater than 100");
    }
    SerializerId::Custom(value)
  }

  pub fn is_custom(&self) -> bool {
    matches!(self, SerializerId::Custom(_))
  }
}

impl Display for SerializerId {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      SerializerId::None => write!(f, "None:0"),
      SerializerId::Proto => write!(f, "Proto:1"),
      SerializerId::Json => write!(f, "Json:2"),
      SerializerId::Custom(value) => write!(f, "Custom:{}", value),
    }
  }
}

impl From<SerializerId> for u32 {
  fn from(id: SerializerId) -> Self {
    match id {
      SerializerId::None => 0,
      SerializerId::Proto => 1,
      SerializerId::Json => 2,
      SerializerId::Custom(value) => value,
    }
  }
}

impl TryFrom<u32> for SerializerId {
  type Error = String;

  fn try_from(value: u32) -> Result<Self, Self::Error> {
    match value {
      0 => Ok(SerializerId::None),
      1 => Ok(SerializerId::Proto),
      2 => Ok(SerializerId::Json),
      _ => {
        if value > 100 {
          Ok(SerializerId::Custom(value))
        } else {
          Err(format!("Invalid SerializerId value: {}", value))
        }
      }
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SerializerKey {
  serializer_id: SerializerId,
  type_name: String,
  is_any: bool,
}

impl SerializerKey {
  pub fn new(serializer_id: SerializerId, type_name: String, is_any: bool) -> Self {
    Self {
      serializer_id,
      type_name,
      is_any,
    }
  }
}

impl Hash for SerializerKey {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.serializer_id.hash(state);
    self.type_name.hash(state);
  }
}

static SERIALIZERS: Lazy<DashMap<SerializerKey, Arc<dyn Any + Send + Sync>>> = Lazy::new(DashMap::new);

pub fn register_serializer<T: 'static>(
  serializer_id: SerializerId,
  serializer: Arc<dyn Serializer<T>>,
) -> Result<(), SerializerError> {
  tracing::debug!(
    "Registering serializer: serializer_id = {}, type_name = {}",
    serializer_id.to_string(),
    serializer.get_type_name()
  );
  let key = SerializerKey::new(serializer_id, serializer.get_type_name(), false);
  let mut h = DefaultHasher::new();
  key.hash(&mut h);
  tracing::debug!("register_serializer: key hash = {:?}", h.finish());
  SERIALIZERS.insert(key, Arc::new(serializer) as Arc<dyn Any + Send + Sync>);
  Ok(())
}

pub fn register_serializer_any(
  serializer_id: SerializerId,
  serializer: Arc<dyn SerializerAny>,
) -> Result<(), SerializerError> {
  tracing::debug!(
    "Registering serializer: serializer_id = {}, type_name = {}",
    serializer_id.to_string(),
    serializer.type_name()
  );
  let key = SerializerKey::new(serializer_id, serializer.type_name(), true);
  let mut h = DefaultHasher::new();
  key.hash(&mut h);
  tracing::debug!("register_serializer_any: key hash = {:?}", h.finish());
  SERIALIZERS.insert(key, Arc::new(serializer) as Arc<dyn Any + Send + Sync>);
  Ok(())
}

pub trait AnyDowncastExt {
  fn downcast_arc<T: Any + Send + Sync>(self) -> Result<Arc<T>, Arc<dyn Any + Send + Sync>>;
}

impl AnyDowncastExt for Arc<dyn Any + Send + Sync> {
  fn downcast_arc<T: Any + Send + Sync>(self) -> Result<Arc<T>, Arc<dyn Any + Send + Sync>> {
    Arc::downcast(self)
  }
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn find_serializer<T: 'static>(serializer_id: &SerializerId, type_name: &str) -> Option<Arc<dyn Serializer<T>>> {
  let key = SerializerKey::new(serializer_id.clone(), type_name.to_string(), false);
  SERIALIZERS
    .get(&key)
    .and_then(|s| s.clone().downcast_arc::<Arc<dyn Serializer<T>>>().ok())
    .map(|arc| arc.as_ref().clone())
}

pub fn find_serializer_any(serializer_id: &SerializerId, type_name: &str) -> Option<Arc<dyn SerializerAny>> {
  let key = SerializerKey::new(serializer_id.clone(), type_name.to_string(), true);
  let mut h = DefaultHasher::new();
  key.hash(&mut h);
  tracing::debug!("find_serializer_any: key hash = {:?}", h.finish());
  let value_opt = SERIALIZERS.get(&key);
  tracing::debug!("find_serializer_any: value_opt = {:?}", value_opt);
  value_opt
    .and_then(|s| s.clone().downcast_arc::<Arc<dyn SerializerAny>>().ok())
    .map(|arc| arc.as_ref().clone())
}

pub fn find_serializer_any_all(type_name: &str) -> Option<Arc<dyn SerializerAny>> {
  tracing::debug!(
    "find_serializer_any_all: type_name: {}, SERIALIZERS = {:?}",
    type_name,
    SERIALIZERS
  );
  for s_id in [SerializerId::Proto, SerializerId::Json].iter() {
    let key = SerializerKey::new(s_id.clone(), type_name.to_string(), true);
    tracing::debug!("find_serializer_any_all: key = {:?}", key);
    let result = SERIALIZERS
      .get(&key)
      .and_then(|s| s.clone().downcast_arc::<Arc<dyn SerializerAny>>().ok())
      .map(|arc| arc.as_ref().clone());
    if result.is_some() {
      tracing::debug!("find_serializer_any_all: found");
      return result;
    }
  }
  tracing::debug!("find_serializer_any_all: not found");
  None
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn serialize<T: 'static>(msg: &T, serializer_id: &SerializerId) -> Result<Vec<u8>, SerializerError> {
  let serializer =
    find_serializer::<T>(serializer_id, std::any::type_name::<T>()).ok_or_else(SerializerError::unknown_type)?;
  serializer.serialize(msg)
}

pub fn serialize_any(msg: &dyn Any, serializer_id: &SerializerId, type_name: &str) -> Result<Vec<u8>, SerializerError> {
  tracing::debug!(
    "serialize_any: serializer_id = {}, type_name = {}",
    serializer_id,
    type_name
  );
  if *serializer_id == SerializerId::None {
    let serializer = find_serializer_any_all(type_name).ok_or_else(SerializerError::unknown_type)?;
    return serializer.serialize_any(msg);
  }
  let serializer_opt = find_serializer_any(serializer_id, type_name);
  let serializer = serializer_opt.ok_or_else(SerializerError::unknown_type)?;
  serializer.serialize_any(msg)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn deserialize<T: 'static>(bytes: &[u8], serializer_id: &SerializerId) -> Result<T, SerializerError> {
  let serializer =
    find_serializer::<T>(serializer_id, std::any::type_name::<T>()).ok_or_else(SerializerError::unknown_type)?;
  serializer.deserialize(bytes)
}

pub fn deserialize_any(
  bytes: &[u8],
  serializer_id: &SerializerId,
  type_name: &str,
) -> Result<Arc<dyn Any + Send + Sync>, SerializerError> {
  if *serializer_id == SerializerId::None {
    let serializer = find_serializer_any_all(type_name).ok_or_else(SerializerError::unknown_type)?;
    return serializer.deserialize_any(bytes);
  }
  let serializer = find_serializer_any(serializer_id, type_name).ok_or_else(SerializerError::unknown_type)?;
  serializer.deserialize_any(bytes)
}

pub fn deserialize_message(
  bytes: &[u8],
  serializer_id: &SerializerId,
  type_name: &str,
) -> Result<Arc<dyn Message>, SerializerError> {
  if *serializer_id == SerializerId::None {
    let serializer = find_serializer_any_all(type_name).ok_or_else(SerializerError::unknown_type)?;
    return serializer.deserialize_message(bytes);
  }
  let serializer = find_serializer_any(serializer_id, type_name).ok_or_else(SerializerError::unknown_type)?;
  serializer.deserialize_message(bytes)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn initialize_serializers<T>() -> Result<(), SerializerError>
where
  T: Message + Default + ProstMessage + Serialize + for<'de> Deserialize<'de> + Send + Sync + 'static, {
  initialize_proto_serializers::<T>()?;
  initialize_json_serializers::<T>()?;
  Ok(())
}

pub fn initialize_json_serializers<T: Message + Serialize + for<'de> Deserialize<'de> + Send + Sync + 'static>(
) -> Result<(), SerializerError> {
  register_serializer(SerializerId::Json, Arc::new(JsonSerializer::<T>::default()))?;
  register_serializer_any(SerializerId::Json, Arc::new(JsonSerializer::<T>::default()))?;
  Ok(())
}

pub fn initialize_proto_serializers<T: Message + Default + ProstMessage + Send + Sync + 'static>(
) -> Result<(), SerializerError> {
  register_serializer(SerializerId::Proto, Arc::new(ProtoSerializer::<T>::default()))?;
  register_serializer_any(SerializerId::Proto, Arc::new(ProtoSerializer::<T>::default()))?;
  Ok(())
}
