mod dyn_message;
mod metadata_table;

pub use dyn_message::DynMessage;
pub use metadata_table::{discard_metadata, store_metadata, take_metadata, MetadataKey, MetadataStorageMode};
