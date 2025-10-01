use std::time::Duration;

#[derive(Default, Debug, Clone)]
pub struct ServerConfig {
  pub concurrency_limit_per_connection: Option<usize>,
  pub timeout: Option<Duration>,
  pub initial_stream_window_size: Option<u32>,
  pub initial_connection_window_size: Option<u32>,
  pub max_concurrent_streams: Option<u32>,
  pub http2_keepalive_interval: Option<Duration>,
  pub http2_keepalive_timeout: Option<Duration>,
  pub http2_adaptive_window: Option<bool>,
  pub http2_max_pending_accept_reset_streams: Option<usize>,
  pub tcp_nodelay: bool,
  pub tcp_keepalive: Option<Duration>,
  pub http2_max_header_list_size: Option<u32>,
  pub max_frame_size: Option<u32>,
  pub accept_http1: bool,
}
