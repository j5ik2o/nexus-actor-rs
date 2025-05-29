use async_trait::async_trait;
use nexus_actor_core_rs::actor::actor_system::ActorSystem;
use nexus_actor_core_rs::actor::context::{SenderPart, SpawnerPart};
use nexus_actor_core_rs::actor::core::{ActorError, Props};
use nexus_actor_core_rs::actor::core_types::{BaseActor, BaseActorError, BaseContext, Message, MigrationHelpers};
use nexus_actor_core_rs::actor::message::MessageHandle;
use std::any::Any;
use std::fmt::Debug;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

// メッセージ定義
#[derive(Debug, Clone)]
struct CreateChild {
  name: String,
  with_props: bool,
}

#[derive(Debug, Clone)]
struct ChildMessage;

impl Message for CreateChild {
  fn eq_message(&self, other: &dyn Message) -> bool {
    if let Some(other_msg) = other.as_any().downcast_ref::<CreateChild>() {
      self.name == other_msg.name
    } else {
      false
    }
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }

  fn get_type_name(&self) -> String {
    "CreateChild".to_string()
  }
}

impl Message for ChildMessage {
  fn eq_message(&self, other: &dyn Message) -> bool {
    other.as_any().downcast_ref::<ChildMessage>().is_some()
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }

  fn get_type_name(&self) -> String {
    "ChildMessage".to_string()
  }
}

// 子アクター
#[derive(Debug)]
struct ChildActor {
  name: String,
  counter: Arc<AtomicUsize>,
}

#[async_trait]
impl BaseActor for ChildActor {
  async fn handle(&mut self, _context: &dyn BaseContext) -> Result<(), BaseActorError> {
    let count = self.counter.fetch_add(1, Ordering::SeqCst) + 1;
    println!("[Child {}] Received message #{}", self.name, count);
    Ok(())
  }

  async fn pre_start(&mut self, _context: &dyn BaseContext) -> Result<(), BaseActorError> {
    println!("[Child {}] Started!", self.name);
    Ok(())
  }
}

// 親アクター（従来のActorトレイトを使用してPropsアクセスを簡単にする）
#[derive(Debug)]
struct ParentActor {
  child_counter: Arc<AtomicUsize>,
}

#[async_trait]
impl nexus_actor_core_rs::actor::core::Actor for ParentActor {
  async fn receive(
    &mut self,
    mut context: nexus_actor_core_rs::actor::context::ContextHandle,
  ) -> Result<(), ActorError> {
    use nexus_actor_core_rs::actor::context::{MessagePart, SpawnerPart};

    let msg = context.get_message_handle().await;

    if let Some(create_child) = msg.to_typed::<CreateChild>() {
      println!(
        "[Parent] Creating child '{}' with_props={}",
        create_child.name, create_child.with_props
      );

      if create_child.with_props {
        // Propsを直接使って子アクターを作成
        let counter = self.child_counter.clone();
        let child_name = create_child.name.clone();

        let child_props = Props::from_async_actor_receiver(move |ctx| {
          let actor = ChildActor {
            name: child_name.clone(),
            counter: counter.clone(),
          };
          let mut migrated = nexus_actor_core_rs::actor::core_types::MigratedActor::new(actor);
          async move { migrated.handle(ctx).await }
        })
        .await;

        let mut ctx_clone = context.clone();
        let child_pid = ctx_clone
          .spawn_named(child_props, &create_child.name)
          .await
          .expect("Failed to spawn child actor");

        // 子アクターにメッセージを送信
        context.send(child_pid, MessageHandle::new(ChildMessage)).await;
      } else {
        // BaseActorを直接作成（MigrationHelpersを使用）
        let counter = self.child_counter.clone();
        let child_name = create_child.name.clone();

        let child_props = MigrationHelpers::props_from_base_actor_fn(move || ChildActor {
          name: child_name.clone(),
          counter: counter.clone(),
        })
        .await;

        let mut ctx_clone = context.clone();
        let child_pid = ctx_clone
          .spawn_named(child_props, &create_child.name)
          .await
          .expect("Failed to spawn child actor");

        // 子アクターにメッセージを送信
        context.send(child_pid, MessageHandle::new(ChildMessage)).await;
      }
    }

    Ok(())
  }
}

#[tokio::main]
async fn main() {
  println!("=== BaseActor with Props Example ===\n");

  // アクターシステムの作成
  let actor_system = ActorSystem::new().await.unwrap();
  let mut root_context = actor_system.get_root_context().await;

  // 親アクターの作成
  let child_counter = Arc::new(AtomicUsize::new(0));
  let child_counter_clone = child_counter.clone();
  // 親アクターをspawn（従来のActorなのでそのままPropsを作成）
  let parent_props = Props::from_async_actor_producer(move |_| {
    let parent_actor = ParentActor {
      child_counter: child_counter_clone.clone(),
    };
    async move { parent_actor }
  })
  .await;
  let parent_pid = root_context.spawn(parent_props).await;

  println!("Creating children using different methods...\n");

  // 方法1: ActorFactoryを使った子アクター作成
  let msg1 = MessageHandle::new(CreateChild {
    name: "child-factory".to_string(),
    with_props: false,
  });
  root_context.send(parent_pid.clone(), msg1).await;

  // 少し待つ
  tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

  // 方法2: Propsを使った子アクター作成
  let msg2 = MessageHandle::new(CreateChild {
    name: "child-props".to_string(),
    with_props: true,
  });
  root_context.send(parent_pid.clone(), msg2).await;

  // 処理を待つ
  tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

  println!(
    "\nTotal child messages processed: {}",
    child_counter.load(Ordering::SeqCst)
  );
  println!("\n=== Example completed! ===");
}
