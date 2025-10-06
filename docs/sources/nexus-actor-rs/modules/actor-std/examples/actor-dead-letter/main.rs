use clap::Parser;
use governor::{Quota, RateLimiter};
use nexus_actor_std_rs::actor::actor_system::ActorSystem;
use nexus_actor_std_rs::actor::context::SenderPart;
use nexus_actor_std_rs::actor::message::{Message, MessageHandle};
use nexus_actor_std_rs::actor::{Config, ConfigOption};
use nexus_actor_std_rs::Message;
use std::env;
use std::num::NonZeroU32;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
  #[clap(long, default_value = "1000000")]
  rate: u32,

  #[clap(long, default_value = "5")]
  throttle: u32,

  #[clap(long, default_value = "10")]
  duration: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Message)]
struct Hello {
  who: String,
}

#[tokio::main]
async fn main() {
  env::set_var("RUST_LOG", "actor_dead_letter=info");
  tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .init();

  let args = Args::parse();
  let btn = Arc::new(AtomicBool::new(true));
  let cloned_btn = btn.clone();

  let config = Config::from([
    ConfigOption::with_dead_letter_throttle_count(10),
    ConfigOption::with_dead_letter_throttle_interval(Duration::from_secs(1)),
  ]);

  let system = ActorSystem::new_with_config(config).await.unwrap();
  let mut root = system.get_root_context().await;
  tokio::spawn(async move {
    sleep(Duration::from_secs(args.duration)).await;
    btn.store(false, Ordering::SeqCst);
  });

  let invalid_pid = system.new_local_pid("unknown").await;
  let limiter = RateLimiter::direct(Quota::per_second(
    NonZeroU32::new(args.rate).unwrap_or(NonZeroU32::new(1).unwrap()),
  ));

  tracing::info!("started");

  tokio::spawn(async move {
    while cloned_btn.load(Ordering::SeqCst) {
      let msg = Hello {
        who: "deadletter".to_string(),
      };
      root.send(invalid_pid.clone(), MessageHandle::new(msg)).await;
      limiter.until_ready().await;
    }
  });

  sleep(Duration::from_secs(1)).await;

  tracing::info!("done");
}
