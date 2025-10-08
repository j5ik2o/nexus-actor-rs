//! Behaviors DSL を使った簡単な挨拶アクターのサンプル。
//!
//! `Behaviors::setup` で状態（挨拶回数）を初期化し、
//! `Behaviors::receive` でメッセージごとの遷移を定義しています。

use nexus_actor_core_rs::{ActorSystem, Behaviors, MailboxOptions, Props};
use nexus_actor_std_rs::TokioMailboxFactory;
use nexus_utils_std_rs::Element;

#[derive(Clone, Debug)]
enum Command {
  Greet(String),
  Report,
  Stop,
}

impl Element for Command {}

#[tokio::main(flavor = "current_thread")]
async fn main() {
  let mut system: ActorSystem<Command, _> = ActorSystem::new(TokioMailboxFactory);
  let mut root = system.root_context();

  let greeter_props = Props::with_behavior(MailboxOptions::default(), || {
    Behaviors::setup(|_| {
      let mut greeted = 0usize;

      Behaviors::receive(move |ctx, msg: Command| match msg {
        Command::Greet(name) => {
          greeted += 1;
          ctx.log().info(|| format!("received greeting for {name}"));
          println!("actor {:?} says: Hello, {}!", ctx.actor_id(), name);
          Behaviors::same()
        }
        Command::Report => {
          ctx.log().debug(|| format!("reporting {greeted} greetings"));
          println!("actor {:?} greeted {} people", ctx.actor_id(), greeted);
          Behaviors::same()
        }
        Command::Stop => {
          ctx.log().warn(|| format!("stopping after {greeted} greetings"));
          println!("actor {:?} is stopping after {} greetings", ctx.actor_id(), greeted);
          Behaviors::transition(Behaviors::stopped())
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
