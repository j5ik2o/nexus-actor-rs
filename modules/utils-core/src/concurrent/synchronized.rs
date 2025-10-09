use alloc::boxed::Box;
use async_trait::async_trait;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};

/// ガードオブジェクトをラップするハンドル
///
/// このハンドルは、ミューテックスやRwLockのガードを保持し、
/// `Deref`や`DerefMut`を通じて中身にアクセスできるようにします。
#[derive(Debug)]
pub struct GuardHandle<G> {
  guard: G,
}

impl<G> GuardHandle<G> {
  /// 新しい`GuardHandle`を作成します。
  ///
  /// # Arguments
  ///
  /// * `guard` - ラップするガードオブジェクト
  pub fn new(guard: G) -> Self {
    Self { guard }
  }

  /// ガードオブジェクトを取り出します。
  ///
  /// # Returns
  ///
  /// 内部のガードオブジェクト
  pub fn into_inner(self) -> G {
    self.guard
  }
}

impl<G> Deref for GuardHandle<G> {
  type Target = G;

  fn deref(&self) -> &Self::Target {
    &self.guard
  }
}

impl<G> DerefMut for GuardHandle<G>
where
  G: DerefMut,
{
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.guard
  }
}

/// 非同期ミューテックス風プリミティブのバックエンドトレイト
///
/// このトレイトは、`Synchronized`型で使用される非同期ミューテックスの
/// バックエンド実装を定義します。具体的な実装は、Tokio、async-std、
/// あるいはカスタムランタイムによって提供されます。
#[async_trait(?Send)]
pub trait SynchronizedMutexBackend<T: ?Sized> {
  /// ロックを取得した際に返されるガード型
  type Guard<'a>: Deref<Target = T> + DerefMut + 'a
  where
    Self: 'a;

  /// 指定された値で新しいバックエンドを作成します。
  ///
  /// # Arguments
  ///
  /// * `value` - ミューテックスに保護される初期値
  fn new(value: T) -> Self
  where
    T: Sized;

  /// ミューテックスをロックし、ガードを取得します。
  ///
  /// # Returns
  ///
  /// 保護された値へのアクセスを提供するガード
  async fn lock(&self) -> Self::Guard<'_>;
}

/// 非同期読み取り/書き込みロックプリミティブのバックエンドトレイト
///
/// このトレイトは、`SynchronizedRw`型で使用される非同期RwLockの
/// バックエンド実装を定義します。読み取りアクセスと書き込みアクセスを
/// 分離することで、複数の読み取りと単一の書き込みを可能にします。
#[async_trait(?Send)]
pub trait SynchronizedRwBackend<T: ?Sized> {
  /// 読み取りロックを取得した際に返されるガード型
  type ReadGuard<'a>: Deref<Target = T> + 'a
  where
    Self: 'a;

  /// 書き込みロックを取得した際に返されるガード型
  type WriteGuard<'a>: Deref<Target = T> + DerefMut + 'a
  where
    Self: 'a;

  /// 指定された値で新しいバックエンドを作成します。
  ///
  /// # Arguments
  ///
  /// * `value` - RwLockに保護される初期値
  fn new(value: T) -> Self
  where
    T: Sized;

  /// 読み取りロックを取得します。
  ///
  /// # Returns
  ///
  /// 保護された値への読み取り専用アクセスを提供するガード
  async fn read(&self) -> Self::ReadGuard<'_>;

  /// 書き込みロックを取得します。
  ///
  /// # Returns
  ///
  /// 保護された値への排他的アクセスを提供するガード
  async fn write(&self) -> Self::WriteGuard<'_>;
}

/// バックエンド抽象化を提供する非同期同期プリミティブ
///
/// `Synchronized`は、ミューテックス風のバックエンドを使用して値への
/// 排他的アクセスを提供します。バックエンドは`SynchronizedMutexBackend`
/// トレイトを実装することで、異なるランタイムやカスタム実装に対応できます。
///
/// # 型パラメータ
///
/// * `B` - 使用するバックエンド実装
/// * `T` - 保護される値の型
#[derive(Debug)]
pub struct Synchronized<B, T: ?Sized>
where
  B: SynchronizedMutexBackend<T>,
{
  backend: B,
  _marker: PhantomData<T>,
}

impl<B, T> Synchronized<B, T>
where
  T: ?Sized,
  B: SynchronizedMutexBackend<T>,
{
  /// 新しい`Synchronized`を指定された値で作成します。
  ///
  /// # Arguments
  ///
  /// * `value` - 保護する初期値
  pub fn new(value: T) -> Self
  where
    T: Sized,
  {
    Self {
      backend: B::new(value),
      _marker: PhantomData,
    }
  }

  /// 既存のバックエンドから`Synchronized`を作成します。
  ///
  /// # Arguments
  ///
  /// * `backend` - 使用するバックエンドインスタンス
  pub fn from_backend(backend: B) -> Self {
    Self {
      backend,
      _marker: PhantomData,
    }
  }

  /// 内部のバックエンドへの参照を取得します。
  ///
  /// # Returns
  ///
  /// バックエンドへの不変参照
  pub fn backend(&self) -> &B {
    &self.backend
  }

  /// ロックを取得し、指定された関数を実行します（読み取り用）。
  ///
  /// # Arguments
  ///
  /// * `f` - ガードへの参照を受け取る関数
  ///
  /// # Returns
  ///
  /// 関数`f`の戻り値
  pub async fn read<R>(&self, f: impl FnOnce(&B::Guard<'_>) -> R) -> R {
    let guard = self.backend.lock().await;
    f(&guard)
  }

  /// ロックを取得し、指定された関数を実行します（書き込み用）。
  ///
  /// # Arguments
  ///
  /// * `f` - ガードへの可変参照を受け取る関数
  ///
  /// # Returns
  ///
  /// 関数`f`の戻り値
  pub async fn write<R>(&self, f: impl FnOnce(&mut B::Guard<'_>) -> R) -> R {
    let mut guard = self.backend.lock().await;
    f(&mut guard)
  }

  /// ロックを取得し、ガードハンドルを返します。
  ///
  /// このメソッドは、ガードを保持し続ける必要がある場合に使用します。
  ///
  /// # Returns
  ///
  /// ガードオブジェクトをラップした`GuardHandle`
  pub async fn lock(&self) -> GuardHandle<B::Guard<'_>> {
    let guard = self.backend.lock().await;
    GuardHandle::new(guard)
  }
}

impl<B, T> Default for Synchronized<B, T>
where
  T: Default,
  B: SynchronizedMutexBackend<T>,
{
  fn default() -> Self {
    Self::new(T::default())
  }
}

impl<B, T> From<T> for Synchronized<B, T>
where
  T: Sized,
  B: SynchronizedMutexBackend<T>,
{
  fn from(value: T) -> Self {
    Self::new(value)
  }
}

/// バックエンド抽象化を提供する非同期読み取り/書き込み同期プリミティブ
///
/// `SynchronizedRw`は、RwLock風のバックエンドを使用して値への
/// 読み取りと書き込みのアクセスを分離して提供します。複数の読み取りを
/// 同時に許可しつつ、書き込みは排他的に行われます。
///
/// # 型パラメータ
///
/// * `B` - 使用するバックエンド実装
/// * `T` - 保護される値の型
#[derive(Debug)]
pub struct SynchronizedRw<B, T: ?Sized>
where
  B: SynchronizedRwBackend<T>,
{
  backend: B,
  _marker: PhantomData<T>,
}

impl<B, T> SynchronizedRw<B, T>
where
  T: ?Sized,
  B: SynchronizedRwBackend<T>,
{
  /// 新しい`SynchronizedRw`を指定された値で作成します。
  ///
  /// # Arguments
  ///
  /// * `value` - 保護する初期値
  pub fn new(value: T) -> Self
  where
    T: Sized,
  {
    Self {
      backend: B::new(value),
      _marker: PhantomData,
    }
  }

  /// 既存のバックエンドから`SynchronizedRw`を作成します。
  ///
  /// # Arguments
  ///
  /// * `backend` - 使用するバックエンドインスタンス
  pub fn from_backend(backend: B) -> Self {
    Self {
      backend,
      _marker: PhantomData,
    }
  }

  /// 内部のバックエンドへの参照を取得します。
  ///
  /// # Returns
  ///
  /// バックエンドへの不変参照
  pub fn backend(&self) -> &B {
    &self.backend
  }

  /// 読み取りロックを取得し、指定された関数を実行します。
  ///
  /// # Arguments
  ///
  /// * `f` - 読み取りガードへの参照を受け取る関数
  ///
  /// # Returns
  ///
  /// 関数`f`の戻り値
  pub async fn read<R>(&self, f: impl FnOnce(&B::ReadGuard<'_>) -> R) -> R {
    let guard = self.backend.read().await;
    f(&guard)
  }

  /// 書き込みロックを取得し、指定された関数を実行します。
  ///
  /// # Arguments
  ///
  /// * `f` - 書き込みガードへの可変参照を受け取る関数
  ///
  /// # Returns
  ///
  /// 関数`f`の戻り値
  pub async fn write<R>(&self, f: impl FnOnce(&mut B::WriteGuard<'_>) -> R) -> R {
    let mut guard = self.backend.write().await;
    f(&mut guard)
  }

  /// 読み取りロックを取得し、ガードハンドルを返します。
  ///
  /// このメソッドは、読み取りガードを保持し続ける必要がある場合に使用します。
  ///
  /// # Returns
  ///
  /// 読み取りガードオブジェクトをラップした`GuardHandle`
  pub async fn read_guard(&self) -> GuardHandle<B::ReadGuard<'_>> {
    let guard = self.backend.read().await;
    GuardHandle::new(guard)
  }

  /// 書き込みロックを取得し、ガードハンドルを返します。
  ///
  /// このメソッドは、書き込みガードを保持し続ける必要がある場合に使用します。
  ///
  /// # Returns
  ///
  /// 書き込みガードオブジェクトをラップした`GuardHandle`
  pub async fn write_guard(&self) -> GuardHandle<B::WriteGuard<'_>> {
    let guard = self.backend.write().await;
    GuardHandle::new(guard)
  }
}

impl<B, T> Default for SynchronizedRw<B, T>
where
  T: Default,
  B: SynchronizedRwBackend<T>,
{
  fn default() -> Self {
    Self::new(T::default())
  }
}

impl<B, T> From<T> for SynchronizedRw<B, T>
where
  T: Sized,
  B: SynchronizedRwBackend<T>,
{
  fn from(value: T) -> Self {
    Self::new(value)
  }
}
