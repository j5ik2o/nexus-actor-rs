use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex, RawMutex};
use nexus_utils_core_rs::{
  QueueSize, Stack, StackBase, StackBuffer, StackError, StackMut, StackStorage, StackStorageBackend, StateCell,
};

use crate::sync::{ArcShared, ArcStateCell};

/// `Arc`-based stack with configurable mutex backend
///
/// This stack provides LIFO (Last In, First Out) semantics using `Arc` for
/// thread-safe reference counting. The mutex backend is configurable via the `RM`
/// type parameter, allowing selection between `NoopRawMutex` for single-threaded
/// or interrupt-free contexts, and `CriticalSectionRawMutex` for interrupt-safe
/// critical sections.
///
/// # Type Parameters
///
/// * `T` - Element type stored in the stack
/// * `RM` - Raw mutex type (defaults to `NoopRawMutex`)
#[derive(Debug)]
pub struct ArcStack<T, RM = NoopRawMutex>
where
  RM: RawMutex, {
  inner: Stack<ArcShared<StackStorageBackend<ArcShared<ArcStateCell<StackBuffer<T>, RM>>>>, T>,
}

/// Type alias for `ArcStack` using `NoopRawMutex`
///
/// Suitable for single-threaded or interrupt-free contexts where no locking is required.
pub type ArcLocalStack<T> = ArcStack<T, NoopRawMutex>;

/// Type alias for `ArcStack` using `CriticalSectionRawMutex`
///
/// Provides interrupt-safe critical section protection for multi-threaded embedded contexts.
pub type ArcCsStack<T> = ArcStack<T, CriticalSectionRawMutex>;

impl<T, RM> ArcStack<T, RM>
where
  RM: RawMutex,
{
  /// Creates a new stack with unlimited capacity
  ///
  /// # Returns
  ///
  /// A new `ArcStack` instance with no capacity limit
  ///
  /// # Examples
  ///
  /// ```ignore
  /// use nexus_utils_embedded_rs::ArcStack;
  ///
  /// let stack: ArcStack<i32> = ArcStack::new();
  /// ```
  pub fn new() -> Self {
    let storage = ArcShared::new(ArcStateCell::new(StackBuffer::new()));
    let backend = ArcShared::new(StackStorageBackend::new(storage));
    Self {
      inner: Stack::new(backend),
    }
  }

  /// Creates a new stack with the specified capacity
  ///
  /// # Arguments
  ///
  /// * `capacity` - Maximum number of elements the stack can hold
  ///
  /// # Returns
  ///
  /// A new `ArcStack` instance with the specified capacity limit
  ///
  /// # Examples
  ///
  /// ```ignore
  /// use nexus_utils_embedded_rs::ArcStack;
  ///
  /// let stack: ArcStack<i32> = ArcStack::with_capacity(10);
  /// ```
  pub fn with_capacity(capacity: usize) -> Self {
    let stack = Self::new();
    stack.set_capacity(Some(capacity));
    stack
  }

  /// Sets the capacity of the stack
  ///
  /// # Arguments
  ///
  /// * `capacity` - Optional maximum number of elements. `None` for unlimited capacity
  ///
  /// # Examples
  ///
  /// ```ignore
  /// use nexus_utils_embedded_rs::ArcStack;
  ///
  /// let stack: ArcStack<i32> = ArcStack::new();
  /// stack.set_capacity(Some(5)); // Limit to 5 elements
  /// stack.set_capacity(None);    // Remove capacity limit
  /// ```
  pub fn set_capacity(&self, capacity: Option<usize>) {
    self.inner.set_capacity(capacity);
  }

  /// Pushes a value onto the stack
  ///
  /// # Arguments
  ///
  /// * `value` - The value to push onto the stack
  ///
  /// # Returns
  ///
  /// * `Ok(())` - If the value was successfully pushed
  /// * `Err(StackError<T>)` - If the stack is full or closed
  ///
  /// # Examples
  ///
  /// ```ignore
  /// use nexus_utils_embedded_rs::ArcStack;
  ///
  /// let stack: ArcStack<i32> = ArcStack::with_capacity(2);
  /// stack.push(1).unwrap();
  /// stack.push(2).unwrap();
  /// assert!(stack.push(3).is_err()); // Stack is full
  /// ```
  pub fn push(&self, value: T) -> Result<(), StackError<T>> {
    self.inner.push(value)
  }

  /// Pops a value from the stack
  ///
  /// # Returns
  ///
  /// * `Some(T)` - The top value from the stack
  /// * `None` - If the stack is empty
  pub fn pop(&self) -> Option<T> {
    self.inner.pop()
  }

  /// Peeks at the top value without removing it
  ///
  /// # Returns
  ///
  /// * `Some(T)` - A clone of the top value
  /// * `None` - If the stack is empty
  pub fn peek(&self) -> Option<T>
  where
    T: Clone, {
    self.inner.peek()
  }

  /// Clears all elements from the stack
  pub fn clear(&self) {
    self.inner.clear();
  }

  /// Returns the current number of elements in the stack
  ///
  /// # Returns
  ///
  /// The number of elements as `QueueSize`
  pub fn len(&self) -> QueueSize {
    self.inner.len()
  }

  /// Returns the stack capacity
  ///
  /// # Returns
  ///
  /// * `QueueSize::Limited(n)` - If capacity is limited
  /// * `QueueSize::Limitless` - If capacity is unlimited
  pub fn capacity(&self) -> QueueSize {
    self.inner.capacity()
  }
}

impl<T, RM> Default for ArcStack<T, RM>
where
  RM: RawMutex,
{
  fn default() -> Self {
    Self::new()
  }
}

impl<T, RM> Clone for ArcStack<T, RM>
where
  RM: RawMutex,
{
  fn clone(&self) -> Self {
    Self {
      inner: self.inner.clone(),
    }
  }
}

impl<T, RM> StackBase<T> for ArcStack<T, RM>
where
  RM: RawMutex,
{
  fn len(&self) -> QueueSize {
    self.inner.len()
  }

  fn capacity(&self) -> QueueSize {
    self.inner.capacity()
  }
}

impl<T, RM> StackMut<T> for ArcStack<T, RM>
where
  RM: RawMutex,
{
  fn push(&mut self, value: T) -> Result<(), StackError<T>> {
    self.inner.push(value)
  }

  fn pop(&mut self) -> Option<T> {
    self.inner.pop()
  }

  fn clear(&mut self) {
    self.inner.clear();
  }

  fn peek(&self) -> Option<T>
  where
    T: Clone, {
    self.inner.peek()
  }
}

impl<T, RM> StackStorage<T> for ArcStateCell<StackBuffer<T>, RM>
where
  RM: RawMutex,
{
  fn with_read<R>(&self, f: impl FnOnce(&StackBuffer<T>) -> R) -> R {
    let guard = self.borrow();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut StackBuffer<T>) -> R) -> R {
    let mut guard = self.borrow_mut();
    f(&mut guard)
  }
}

impl<T, RM> StackStorage<T> for ArcShared<ArcStateCell<StackBuffer<T>, RM>>
where
  RM: RawMutex,
{
  fn with_read<R>(&self, f: impl FnOnce(&StackBuffer<T>) -> R) -> R {
    (**self).with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut StackBuffer<T>) -> R) -> R {
    (**self).with_write(f)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::tests::init_arc_critical_section;

  fn prepare() {
    init_arc_critical_section();
  }

  #[test]
  fn arc_stack_push_pop() {
    prepare();
    let stack: ArcStack<u32> = ArcLocalStack::with_capacity(1);
    stack.push(1).unwrap();
    assert!(stack.push(2).is_err());
    assert_eq!(stack.pop(), Some(1));
    assert!(stack.pop().is_none());
  }

  #[test]
  fn arc_stack_handle_operations() {
    prepare();
    let stack: ArcStack<u32> = ArcLocalStack::new();
    stack.push(10).unwrap();
    let cloned = stack.clone();
    cloned.push(11).unwrap();

    assert_eq!(stack.len().to_usize(), 2);
    assert_eq!(cloned.pop(), Some(11));
    assert_eq!(stack.pop(), Some(10));
  }

  #[test]
  fn arc_stack_peek_ref() {
    prepare();
    let stack: ArcStack<u32> = ArcLocalStack::new();
    stack.push(5).unwrap();
    assert_eq!(stack.peek(), Some(5));
    stack.pop();
    assert_eq!(stack.peek(), None);
  }
}
