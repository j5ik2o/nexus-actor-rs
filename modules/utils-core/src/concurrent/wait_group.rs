use alloc::boxed::Box;
use async_trait::async_trait;

/// WaitGroupのバックエンド実装を定義するトレイト
///
/// このトレイトは、異なる非同期ランタイム（Tokio、async-std、組み込みシステムなど）向けに
/// WaitGroupの具体的な実装を提供するために使用されます。
///
/// # 設計思想
///
/// - GoのWaitGroupと同様の並行制御機構を提供
/// - ランタイム非依存の抽象化によるポータビリティ
/// - クローン可能な設計により、複数のタスク間での共有が容易
#[async_trait(?Send)]
pub trait WaitGroupBackend: Clone {
  /// 新しいバックエンドインスタンスを作成
  ///
  /// カウンタは0で初期化されます。
  fn new() -> Self;

  /// 指定されたカウント値で新しいバックエンドインスタンスを作成
  ///
  /// # 引数
  ///
  /// * `count` - 初期カウント値
  fn with_count(count: usize) -> Self;

  /// カウンタに指定された数を加算
  ///
  /// # 引数
  ///
  /// * `n` - 加算する値
  fn add(&self, n: usize);

  /// カウンタを1減算
  ///
  /// カウンタが0になると、`wait()`で待機中のすべてのタスクが再開されます。
  fn done(&self);

  /// カウンタが0になるまで待機
  ///
  /// カウンタが既に0の場合、この関数は即座に返ります。
  async fn wait(&self);
}

/// 複数の並行タスクの完了を待機するための同期プリミティブ
///
/// `WaitGroup`は、Goの`sync.WaitGroup`にインスパイアされた同期機構で、
/// 複数の非同期タスクの完了を待機するために使用されます。
///
/// # 使用パターン
///
/// 1. `add(n)`で待機するタスク数を追加
/// 2. 各タスクで処理完了時に`done()`を呼び出し
/// 3. `wait()`ですべてのタスクの完了を待機
///
/// # 例
///
/// ```rust,ignore
/// let wg = WaitGroup::<TokioWaitGroupBackend>::new();
/// wg.add(3);
///
/// for i in 0..3 {
///     let wg_clone = wg.clone();
///     tokio::spawn(async move {
///         // 何らかの処理
///         wg_clone.done();
///     });
/// }
///
/// wg.wait().await; // すべてのタスクの完了を待機
/// ```
///
/// # 型パラメータ
///
/// * `B` - WaitGroupBackendトレイトを実装する具体的なバックエンド型
#[derive(Clone, Debug)]
pub struct WaitGroup<B>
where
  B: WaitGroupBackend, {
  backend: B,
}

impl<B> WaitGroup<B>
where
  B: WaitGroupBackend,
{
  /// 新しいWaitGroupを作成
  ///
  /// カウンタは0で初期化されます。
  ///
  /// # 戻り値
  ///
  /// 新しいWaitGroupインスタンス
  pub fn new() -> Self {
    Self { backend: B::new() }
  }

  /// 指定されたカウント値で新しいWaitGroupを作成
  ///
  /// # 引数
  ///
  /// * `count` - 初期カウント値（待機するタスク数）
  ///
  /// # 戻り値
  ///
  /// 指定されたカウント値で初期化されたWaitGroupインスタンス
  pub fn with_count(count: usize) -> Self {
    Self {
      backend: B::with_count(count),
    }
  }

  /// カウンタに指定された数を加算
  ///
  /// 新たに開始するタスクの数だけカウンタを増やします。
  ///
  /// # 引数
  ///
  /// * `n` - 加算する値（開始するタスクの数）
  pub fn add(&self, n: usize) {
    self.backend.add(n);
  }

  /// カウンタを1減算
  ///
  /// タスクの完了時に呼び出します。カウンタが0になると、
  /// `wait()`で待機中のすべてのタスクが再開されます。
  pub fn done(&self) {
    self.backend.done();
  }

  /// カウンタが0になるまで非同期に待機
  ///
  /// カウンタが既に0の場合、この関数は即座に返ります。
  /// そうでない場合、すべてのタスクが`done()`を呼び出すまで待機します。
  pub async fn wait(&self) {
    self.backend.wait().await;
  }

  /// バックエンドへの参照を取得
  ///
  /// # 戻り値
  ///
  /// バックエンドインスタンスへの不変参照
  pub fn backend(&self) -> &B {
    &self.backend
  }
}

impl<B> Default for WaitGroup<B>
where
  B: WaitGroupBackend,
{
  fn default() -> Self {
    Self::new()
  }
}
