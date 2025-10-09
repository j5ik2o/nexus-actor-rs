use alloc::boxed::Box;
use async_trait::async_trait;

/// CountDownLatchのバックエンド実装を定義するトレイト
///
/// このトレイトは、異なる環境（標準ライブラリ、組み込み環境など）に対応するため、
/// CountDownLatchの具体的な実装を抽象化します。
///
/// # 例
///
/// ```ignore
/// use async_trait::async_trait;
///
/// #[derive(Clone)]
/// struct MyBackend {
///     // バックエンドの実装詳細
/// }
///
/// #[async_trait(?Send)]
/// impl CountDownLatchBackend for MyBackend {
///     fn new(count: usize) -> Self {
///         // 初期化処理
///     }
///
///     async fn count_down(&self) {
///         // カウントダウン処理
///     }
///
///     async fn wait(&self) {
///         // 待機処理
///     }
/// }
/// ```
#[async_trait(?Send)]
pub trait CountDownLatchBackend: Clone {
  /// 指定されたカウント値でバックエンドを初期化します
  ///
  /// # Arguments
  ///
  /// * `count` - 初期カウント値。0になるまでカウントダウンされます
  fn new(count: usize) -> Self;

  /// カウントを1減らします
  ///
  /// カウントが0になった場合、待機中のすべてのタスクが解放されます。
  async fn count_down(&self);

  /// カウントが0になるまで待機します
  ///
  /// このメソッドは、カウントが0になるまで現在のタスクをブロックします。
  /// 複数のタスクが同時に待機できます。
  async fn wait(&self);
}

/// カウントダウンラッチ同期プリミティブ
///
/// `CountDownLatch`は、指定されたカウントが0になるまで複数のタスクを待機させる同期機構です。
/// Javaの`CountDownLatch`やGoの`WaitGroup`に相当する機能を提供します。
///
/// # 使用例
///
/// ```ignore
/// use nexus_utils_core_rs::CountDownLatch;
///
/// #[tokio::main]
/// async fn main() {
///     let latch = CountDownLatch::<SomeBackend>::new(3);
///
///     // 3つのタスクを起動
///     for i in 0..3 {
///         let latch_clone = latch.clone();
///         tokio::spawn(async move {
///             // 何らかの処理
///             println!("Task {} completed", i);
///             latch_clone.count_down().await;
///         });
///     }
///
///     // すべてのタスクが完了するまで待機
///     latch.wait().await;
///     println!("All tasks completed");
/// }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CountDownLatch<B>
where
  B: CountDownLatchBackend,
{
  backend: B,
}

impl<B> CountDownLatch<B>
where
  B: CountDownLatchBackend,
{
  /// 指定されたカウント値で新しい`CountDownLatch`を作成します
  ///
  /// # Arguments
  ///
  /// * `count` - 初期カウント値。0になるまで`count_down`が呼び出される必要があります
  ///
  /// # 例
  ///
  /// ```ignore
  /// let latch = CountDownLatch::<SomeBackend>::new(5);
  /// ```
  pub fn new(count: usize) -> Self {
    Self { backend: B::new(count) }
  }

  /// カウントを1減らします
  ///
  /// カウントが0になった場合、`wait()`で待機中のすべてのタスクが解放されます。
  /// カウントがすでに0の場合、このメソッドは何も行いません。
  ///
  /// # 例
  ///
  /// ```ignore
  /// latch.count_down().await;
  /// ```
  pub async fn count_down(&self) {
    self.backend.count_down().await;
  }

  /// カウントが0になるまで現在のタスクを待機させます
  ///
  /// カウントがすでに0の場合、このメソッドは即座に返ります。
  /// 複数のタスクが同時に待機でき、カウントが0になるとすべてのタスクが同時に解放されます。
  ///
  /// # 例
  ///
  /// ```ignore
  /// latch.wait().await;
  /// println!("All operations completed");
  /// ```
  pub async fn wait(&self) {
    self.backend.wait().await;
  }

  /// 内部バックエンドへの参照を取得します
  ///
  /// # Returns
  ///
  /// バックエンド実装への不変参照
  pub fn backend(&self) -> &B {
    &self.backend
  }
}

impl<B> Default for CountDownLatch<B>
where
  B: CountDownLatchBackend,
{
  /// カウント値0でデフォルトの`CountDownLatch`を作成します
  ///
  /// このデフォルト実装は、カウント値0のラッチを作成します。
  /// つまり、`wait()`は即座に返ります。
  fn default() -> Self {
    Self::new(0)
  }
}
