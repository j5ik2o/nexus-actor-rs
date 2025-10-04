#[allow(dead_code)]
mod activator_actor;
mod block_list;
mod cluster;
mod config;
mod config_option;
mod endpoint;
mod endpoint_lazy;
mod endpoint_manager;
mod endpoint_reader;
mod endpoint_state;
mod endpoint_supervisor;
mod endpoint_watcher;
mod endpoint_writer;
mod endpoint_writer_mailbox;
mod generated;
mod message_decoder;
mod messages;
mod metrics;
mod remote;
mod remote_process;
mod response_status_code;
mod serializer;
#[cfg(test)]
mod tests;
mod watch_registry;

pub use config::Config;
pub use config_option::{ConfigOption, ConfigOptionError};
pub use endpoint_watcher::EndpointWatcher;
pub use remote::{ActivationHandler, ActivationHandlerError, Remote};
pub use response_status_code::ResponseStatusCode;
pub use serializer::{initialize_json_serializers, initialize_proto_serializers, SerializerId};
pub use watch_registry::WatchRegistry;

pub use nexus_remote_core_rs::{
  BlockListStore, BoxFuture, EndpointHandle, MetricsSink, RemoteRuntime, RemoteRuntimeConfig, RemoteTransport,
  SerializerRegistry, TransportEndpoint, TransportError, TransportErrorKind, TransportListener,
};
