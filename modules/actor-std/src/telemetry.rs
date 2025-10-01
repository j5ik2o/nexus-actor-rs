//! telemetry サポート (tokio-console/tracing 用)

use tracing_subscriber::util::TryInitError;

/// tokio-console を有効化するためのサブスクライバ初期化ヘルパ。
///
/// feature `tokio-console` が無効のときは no-op。
pub fn init_console_subscriber() -> Result<(), TryInitError> {
  #[cfg(feature = "tokio-console")]
  {
    use console_subscriber::ConsoleLayer;
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let console_layer = ConsoleLayer::builder().with_default_env().spawn();

    tracing_subscriber::registry()
      .with(env_filter)
      .with(console_layer)
      .with(fmt::layer())
      .try_init()
  }

  #[cfg(not(feature = "tokio-console"))]
  {
    Ok(())
  }
}
