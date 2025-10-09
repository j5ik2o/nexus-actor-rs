use alloc::boxed::Box;
use async_trait::async_trait;

/// 非同期バリアのバックエンド実装を定義するトレイト。
///
/// このトレイトは、異なる実行環境（Tokio、async-stdなど）に対応した
/// バリア同期機構のバックエンドを実装するために使用されます。
///
/// # 実装要件
///
/// - `Clone`トレイトを実装する必要があります
/// - スレッド送信可能である必要はありません（`?Send`）
///
/// # Examples
///
/// ```ignore
/// use nexus_utils_core_rs::{AsyncBarrierBackend, AsyncBarrier};
///
/// // カスタムバックエンドの実装例
/// #[derive(Clone)]
/// struct MyBackend {
///     // バックエンド固有のフィールド
/// }
///
/// #[async_trait::async_trait(?Send)]
/// impl AsyncBarrierBackend for MyBackend {
///     fn new(count: usize) -> Self {
///         // 初期化処理
///         MyBackend { /* ... */ }
///     }
///
///     async fn wait(&self) {
///         // 待機処理の実装
///     }
/// }
/// ```
#[async_trait(?Send)]
pub trait AsyncBarrierBackend: Clone {
  /// 指定された数のタスクを待機するバックエンドを作成します。
  ///
  /// # Arguments
  ///
  /// * `count` - バリアが解放されるまでに到達する必要があるタスク数
  fn new(count: usize) -> Self;

  /// バリアポイントで待機します。
  ///
  /// このメソッドを呼び出したタスクは、指定された数のタスクが
  /// すべて`wait()`を呼び出すまでブロックされます。
  /// すべてのタスクが到達すると、すべてのタスクが同時に解放されます。
  async fn wait(&self);
}

/// 非同期タスク間の同期バリアを提供する構造体。
///
/// `AsyncBarrier`は、複数の非同期タスクが特定のポイントで同期するための
/// バリア同期機構を提供します。指定された数のタスクがバリアに到達するまで、
/// すべてのタスクが待機し、すべてが到達した時点で一斉に処理を再開します。
///
/// # 型パラメータ
///
/// * `B` - 使用するバックエンド実装（`AsyncBarrierBackend`を実装する型）
///
/// # Examples
///
/// ```ignore
/// use nexus_utils_core_rs::AsyncBarrier;
/// use tokio::task;
///
/// #[tokio::main]
/// async fn main() {
///     // 3つのタスクが到達するまで待機するバリアを作成
///     let barrier = AsyncBarrier::new(3);
///
///     let mut handles = vec![];
///
///     for i in 0..3 {
///         let barrier_clone = barrier.clone();
///         let handle = task::spawn(async move {
///             println!("タスク {} が開始しました", i);
///
///             // バリアで待機
///             barrier_clone.wait().await;
///
///             println!("タスク {} がバリアを通過しました", i);
///         });
///         handles.push(handle);
///     }
///
///     // すべてのタスクの完了を待機
///     for handle in handles {
///         handle.await.unwrap();
///     }
/// }
/// ```
///
/// # 使用例（並行処理の同期）
///
/// ```ignore
/// use nexus_utils_core_rs::AsyncBarrier;
///
/// async fn parallel_computation() {
///     let barrier = AsyncBarrier::new(3);
///
///     // フェーズ1: 各タスクが初期化処理を実行
///     let tasks: Vec<_> = (0..3).map(|i| {
///         let barrier = barrier.clone();
///         tokio::spawn(async move {
///             // 初期化処理
///             println!("タスク {} 初期化中...", i);
///
///             // すべてのタスクが初期化完了するまで待機
///             barrier.wait().await;
///
///             // フェーズ2: すべてが初期化完了後に実行される処理
///             println!("タスク {} メイン処理を実行", i);
///         })
///     }).collect();
///
///     for task in tasks {
///         task.await.unwrap();
///     }
/// }
/// ```
#[derive(Clone, Debug)]
pub struct AsyncBarrier<B>
where
  B: AsyncBarrierBackend, {
  backend: B,
}

impl<B> AsyncBarrier<B>
where
  B: AsyncBarrierBackend,
{
  /// 指定された数のタスクを待機する新しいバリアを作成します。
  ///
  /// # Arguments
  ///
  /// * `count` - バリアが解放されるまでに到達する必要があるタスク数
  ///
  /// # Returns
  ///
  /// 新しい`AsyncBarrier`インスタンス
  ///
  /// # Examples
  ///
  /// ```ignore
  /// use nexus_utils_core_rs::AsyncBarrier;
  ///
  /// // 5つのタスクが到達するまで待機するバリアを作成
  /// let barrier = AsyncBarrier::new(5);
  /// ```
  ///
  /// # Panics
  ///
  /// `count`が0の場合、バックエンド実装によってはパニックする可能性があります。
  pub fn new(count: usize) -> Self {
    Self { backend: B::new(count) }
  }

  /// バリアポイントで待機します。
  ///
  /// このメソッドを呼び出したタスクは、バリアに指定された数のタスクが
  /// すべて`wait()`を呼び出すまで待機状態になります。
  /// すべてのタスクが到達すると、すべてのタスクが同時に解放され、
  /// 処理を続行できます。
  ///
  /// # Examples
  ///
  /// ```ignore
  /// use nexus_utils_core_rs::AsyncBarrier;
  ///
  /// let barrier = AsyncBarrier::new(3);
  ///
  /// // 複数のタスクから呼び出し
  /// let barrier_clone = barrier.clone();
  /// tokio::spawn(async move {
  ///     println!("タスク1: バリアで待機中...");
  ///     barrier_clone.wait().await;
  ///     println!("タスク1: バリアを通過!");
  /// });
  /// ```
  ///
  /// # 動作
  ///
  /// - 最初の`count - 1`個のタスクは、`wait()`呼び出し時に待機状態になります
  /// - `count`番目のタスクが`wait()`を呼び出すと、すべてのタスクが同時に解放されます
  /// - バリアは再利用可能な場合もありますが、それはバックエンド実装に依存します
  pub async fn wait(&self) {
    self.backend.wait().await;
  }

  /// バックエンド実装への参照を取得します。
  ///
  /// このメソッドは、バックエンド固有の機能にアクセスする必要がある場合に使用します。
  /// 通常の使用では、`new()`と`wait()`メソッドのみで十分です。
  ///
  /// # Returns
  ///
  /// バックエンド実装への不変参照
  ///
  /// # Examples
  ///
  /// ```ignore
  /// use nexus_utils_core_rs::AsyncBarrier;
  ///
  /// let barrier = AsyncBarrier::new(3);
  /// let backend = barrier.backend();
  /// // バックエンド固有の操作を実行...
  /// ```
  pub fn backend(&self) -> &B {
    &self.backend
  }
}
