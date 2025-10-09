mod dyn_message;
mod metadata_table;

pub use dyn_message::DynMessage;
#[cfg(test)]
pub(crate) use metadata_table::outstanding_metadata_count;
pub use metadata_table::{store_metadata, take_metadata, MetadataKey};
