//! Behaviors DSL を使った簡単な挨拶アクターのサンプル。
//!
//! `Behaviors::setup` で状態（挨拶回数）を初期化し、
//! `Behaviors::receive` でメッセージごとの遷移を定義しています。

use nexus_actor_core_rs::{ActorSystem, Behaviors, MailboxOptions, Props};
use nexus_actor_std_rs::TokioMailboxFactory;
use nexus_utils_std_rs::Element;
use tracing_subscriber::FmtSubscriber;

#[derive(Clone, Debug)]
enum Command {
  Greet(String),
  Report,
  Stop,
}

impl Element for Command {}

#[tokio::main(flavor = "current_thread")]
async fn main() {
  // tracing サブスクライバを初期化（既に設定済みなら無視）
  let _ = FmtSubscriber::builder()
    .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
    .try_init();

  let mut system: ActorSystem<Command, _> = ActorSystem::new(TokioMailboxFactory);
  let mut root = system.root_context();

  let greeter_props = Props::with_behavior(MailboxOptions::default(), || {
    Behaviors::setup(move |ctx| {
      let mut greeted = 0usize;
      let logger = ctx.log();
      let actor_id = ctx.actor_id();

      Behaviors::receive_message(move |msg: Command| match msg {
        Command::Greet(name) => {
          greeted += 1;
          logger.info(|| format!("actor {:?} says: Hello, {}!", actor_id, name));
          Behaviors::same()
        }
        Command::Report => {
          logger.info(|| format!("actor {:?} greeted {} people", actor_id, greeted));
          Behaviors::same()
        }
        Command::Stop => {
          logger.warn(|| format!("actor {:?} is stopping after {} greetings", actor_id, greeted));
          Behaviors::stopped()
        }
      })
    })
  });

  let greeter = root.spawn(greeter_props).expect("spawn greeter");

  greeter.tell(Command::Greet("Alice".to_owned())).expect("greet Alice");
  greeter.tell(Command::Greet("Bob".to_owned())).expect("greet Bob");
  greeter.tell(Command::Report).expect("report greetings");
  greeter.tell(Command::Stop).expect("stop greeter");

  system.run_until_idle().expect("drain mailbox");
}
