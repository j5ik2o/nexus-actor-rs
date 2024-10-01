use std::time::Duration;

#[derive(Debug, Clone)]
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

impl Default for ServerConfig {
  fn default() -> Self {
    Self {
      concurrency_limit_per_connection: None,
      timeout: None,
      initial_stream_window_size: None,
      initial_connection_window_size: None,
      max_concurrent_streams: None,
      tcp_keepalive: None,
      tcp_nodelay: false,
      http2_keepalive_interval: None,
      http2_keepalive_timeout: None,
      http2_adaptive_window: None,
      http2_max_pending_accept_reset_streams: None,
      http2_max_header_list_size: None,
      max_frame_size: None,
      accept_http1: false,
    }
  }
}
