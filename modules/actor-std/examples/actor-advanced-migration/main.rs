use async_trait::async_trait;
use nexus_actor_std_rs::actor::actor_system::ActorSystem;
use nexus_actor_std_rs::actor::context::{BasePart, ContextHandle, MessagePart, SenderPart, SpawnerPart, StopperPart};
use nexus_actor_std_rs::actor::core::{Actor, ActorError, ErrorReason, ExtendedPid, Props};
use nexus_actor_std_rs::actor::message::{Message, MessageHandle, ResponseHandle};
use nexus_message_derive_rs::Message as DeriveMessage;
use tokio::time::{sleep, Duration};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Clone, PartialEq, DeriveMessage)]
struct ProcessTask {
  id: u32,
  data: String,
}

#[derive(Debug, Clone, PartialEq, DeriveMessage)]
struct TaskResult {
  id: u32,
  result: String,
  processed_by: String,
}

#[derive(Debug, Clone, PartialEq, DeriveMessage)]
struct GetStats;

#[derive(Debug, Clone, PartialEq, DeriveMessage)]
struct Stats {
  tasks_processed: u32,
  worker_count: u32,
}

#[derive(Debug)]
struct WorkerActor {
  id: String,
  tasks_processed: u32,
}

impl WorkerActor {
  fn new(id: String) -> Self {
    Self { id, tasks_processed: 0 }
  }
}

#[async_trait]
impl Actor for WorkerActor {
  async fn pre_start(&mut self, _: ContextHandle) -> Result<(), ActorError> {
    println!("Worker {} started", self.id);
    Ok(())
  }

  async fn receive(&mut self, mut ctx: ContextHandle) -> Result<(), ActorError> {
    let message = ctx.get_message_handle_opt().await.expect("message not found");

    if let Some(task) = message.to_typed::<ProcessTask>() {
      println!("Worker {} processing task {}", self.id, task.id);
      sleep(Duration::from_millis(50)).await;
      self.tasks_processed += 1;

      if let Some(sender) = ctx.get_sender().await {
        let result = TaskResult {
          id: task.id,
          result: format!("processed:{}", task.data),
          processed_by: self.id.clone(),
        };
        ctx.send(sender, MessageHandle::new(result)).await;
      }
    }

    Ok(())
  }

  async fn post_stop(&mut self, _: ContextHandle) -> Result<(), ActorError> {
    println!("Worker {} stopped after {} tasks", self.id, self.tasks_processed);
    Ok(())
  }
}

#[derive(Debug)]
struct SupervisorActor {
  worker_count: u32,
  workers: Vec<ExtendedPid>,
  tasks_delegated: u32,
}

impl SupervisorActor {
  fn new(worker_count: u32) -> Self {
    Self {
      worker_count,
      workers: Vec::new(),
      tasks_delegated: 0,
    }
  }
}

#[async_trait]
impl Actor for SupervisorActor {
  async fn pre_start(&mut self, mut ctx: ContextHandle) -> Result<(), ActorError> {
    println!("Supervisor starting with {} workers", self.worker_count);

    for index in 0..self.worker_count {
      let worker_id = format!("worker-{}", index);
      let worker_id_clone = worker_id.clone();
      let props = Props::from_async_actor_producer(move |_| {
        let worker_id = worker_id_clone.clone();
        async move { WorkerActor::new(worker_id) }
      })
      .await;

      let pid = ctx
        .spawn_named(props, &worker_id)
        .await
        .map_err(|err| ActorError::of_initialization_error(ErrorReason::new(err.to_string(), 0)))?;

      self.workers.push(pid);
    }

    println!("Supervisor started {} workers", self.workers.len());
    Ok(())
  }

  async fn receive(&mut self, mut ctx: ContextHandle) -> Result<(), ActorError> {
    let message = ctx.get_message_handle_opt().await.expect("message not found");

    if let Some(task) = message.to_typed::<ProcessTask>() {
      if self.workers.is_empty() {
        return Ok(());
      }
      let worker_index = (self.tasks_delegated as usize) % self.workers.len();
      let worker_pid = self.workers[worker_index].clone();
      ctx.send(worker_pid, MessageHandle::new(task.clone())).await;
      self.tasks_delegated += 1;
      println!("Supervisor delegated task {} to worker {}", task.id, worker_index);
    } else if message.is_typed::<GetStats>() {
      let stats = Stats {
        tasks_processed: self.tasks_delegated,
        worker_count: self.workers.len() as u32,
      };
      ctx.respond(ResponseHandle::new(stats)).await;
    }

    Ok(())
  }

  async fn post_stop(&mut self, _: ContextHandle) -> Result<(), ActorError> {
    println!("Supervisor stopped after delegating {} tasks", self.tasks_delegated);
    Ok(())
  }
}

#[derive(Debug)]
struct CollectorActor {
  expected_tasks: u32,
  results: Vec<TaskResult>,
}

impl CollectorActor {
  fn new(expected_tasks: u32) -> Self {
    Self {
      expected_tasks,
      results: Vec::new(),
    }
  }
}

#[async_trait]
impl Actor for CollectorActor {
  async fn receive(&mut self, ctx: ContextHandle) -> Result<(), ActorError> {
    let message = ctx.get_message_handle_opt().await.expect("message not found");

    if let Some(result) = message.to_typed::<TaskResult>() {
      println!(
        "Collector received result: task {} -> {} by {}",
        result.id, result.result, result.processed_by
      );
      self.results.push(result.clone());

      if self.results.len() as u32 == self.expected_tasks {
        println!("\n=== All tasks completed! ===");
        for entry in &self.results {
          println!(
            "  Task {} processed by {} => {}",
            entry.id, entry.processed_by, entry.result
          );
        }
      }
    }

    Ok(())
  }
}

#[tokio::main]
async fn main() {
  std::env::set_var("RUST_LOG", "actor_advanced_migration=info");
  tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .init();

  let worker_count = 3;
  let total_tasks = 6u32;

  let system = ActorSystem::new().await.unwrap();
  let mut root = system.get_root_context().await;

  let total_tasks_clone = total_tasks;
  let collector_props = Props::from_async_actor_producer(move |_| {
    let expected = total_tasks_clone;
    async move { CollectorActor::new(expected) }
  })
  .await;
  let collector_pid = root
    .spawn_named(collector_props, "collector")
    .await
    .expect("collector spawn");

  let supervisor_props =
    Props::from_async_actor_producer(move |_| async move { SupervisorActor::new(worker_count) }).await;
  let supervisor_pid = root
    .spawn_named(supervisor_props, "supervisor")
    .await
    .expect("supervisor spawn");

  for seq in 0..total_tasks {
    let task = ProcessTask {
      id: seq,
      data: format!("payload-{}", seq),
    };
    root
      .request_with_custom_sender(supervisor_pid.clone(), MessageHandle::new(task), collector_pid.clone())
      .await;
    sleep(Duration::from_millis(20)).await;
  }

  sleep(Duration::from_secs(1)).await;

  let stats_future = root
    .request_future(
      supervisor_pid.clone(),
      MessageHandle::new(GetStats),
      Duration::from_secs(2),
    )
    .await;

  if let Ok(stats_message) = stats_future.result().await {
    if let Some(stats) = stats_message.to_typed::<Stats>() {
      println!(
        "\nSupervisor reported: {} tasks processed across {} workers",
        stats.tasks_processed, stats.worker_count
      );
    }
  }

  root.stop(&supervisor_pid).await;
  root.stop(&collector_pid).await;

  sleep(Duration::from_millis(200)).await;
}
