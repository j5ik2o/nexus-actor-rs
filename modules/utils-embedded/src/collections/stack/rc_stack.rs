use core::cell::RefCell;

use nexus_utils_core_rs::{
  QueueSize, Stack, StackBase, StackBuffer, StackError, StackMut, StackStorage, StackStorageBackend,
};

use crate::sync::RcShared;

/// `Rc`-based stack storage type alias
///
/// Reference-counted stack storage using `RcShared` and `RefCell`.
type RcStackStorage<T> = RcShared<StackStorageBackend<RcShared<RefCell<StackBuffer<T>>>>>;

/// `Rc`-based stack (Last-In-First-Out data structure)
///
/// This stack is a LIFO (Last-In-First-Out) data structure usable in `no_std` environments.
/// It provides reference-counted shared ownership using `Rc` and `RefCell`.
///
/// # Features
///
/// - **LIFO**: Last added element is retrieved first
/// - **Capacity Limit**: Optional capacity limit can be set
/// - **no_std Support**: Does not require the standard library
/// - **Cloneable**: Multiple handles can be created via `clone()`
///
/// # Performance Characteristics
///
/// - `push`: O(1) (within capacity), O(n) when resizing
/// - `pop`: O(1)
/// - `peek`: O(1)
/// - Memory usage: O(n) (proportional to number of elements)
///
/// # Examples
///
/// ```
/// use nexus_utils_embedded_rs::RcStack;
///
/// // Stack without capacity limit
/// let stack = RcStack::new();
/// stack.push(1).unwrap();
/// stack.push(2).unwrap();
/// assert_eq!(stack.pop(), Some(2));
/// assert_eq!(stack.pop(), Some(1));
///
/// // Stack with capacity limit
/// let limited_stack = RcStack::with_capacity(5);
/// for i in 0..5 {
///     limited_stack.push(i).unwrap();
/// }
/// // Cannot add 6th element
/// assert!(limited_stack.push(5).is_err());
/// ```
#[derive(Debug, Clone)]
pub struct RcStack<T> {
  inner: Stack<RcStackStorage<T>, T>,
}

impl<T> RcStack<T> {
  /// Creates a new stack
  ///
  /// Created without capacity limit.
  ///
  /// # Examples
  ///
  /// ```
  /// use nexus_utils_embedded_rs::RcStack;
  ///
  /// let stack: RcStack<String> = RcStack::new();
  /// ```
  pub fn new() -> Self {
    let storage = RcShared::new(RefCell::new(StackBuffer::new()));
    let backend: RcStackStorage<T> = RcShared::new(StackStorageBackend::new(storage));
    Self {
      inner: Stack::new(backend),
    }
  }

  /// Creates a new stack with the specified capacity limit
  ///
  /// # Arguments
  ///
  /// * `capacity` - Maximum number of elements the stack can hold
  ///
  /// # Examples
  ///
  /// ```
  /// use nexus_utils_embedded_rs::RcStack;
  ///
  /// let stack: RcStack<i32> = RcStack::with_capacity(100);
  /// ```
  pub fn with_capacity(capacity: usize) -> Self {
    let stack = Self::new();
    stack.set_capacity(Some(capacity));
    stack
  }

  /// Sets the stack's capacity limit
  ///
  /// # Arguments
  ///
  /// * `capacity` - `Some(n)` to limit capacity to n elements, `None` to remove capacity limit
  ///
  /// # Examples
  ///
  /// ```
  /// use nexus_utils_embedded_rs::RcStack;
  ///
  /// let stack: RcStack<i32> = RcStack::new();
  /// stack.set_capacity(Some(10)); // Limit capacity to 10
  /// stack.set_capacity(None);     // Remove capacity limit
  /// ```
  pub fn set_capacity(&self, capacity: Option<usize>) {
    self.inner.set_capacity(capacity);
  }

  /// Pushes a value onto the stack
  ///
  /// # Arguments
  ///
  /// * `value` - Value to push onto the stack
  ///
  /// # Returns
  ///
  /// Returns `Ok(())` on success, `Err(StackError::Full(value))` if capacity limit is reached
  ///
  /// # Examples
  ///
  /// ```
  /// use nexus_utils_embedded_rs::RcStack;
  ///
  /// let stack = RcStack::new();
  /// stack.push(42).unwrap();
  /// ```
  pub fn push(&self, value: T) -> Result<(), StackError<T>> {
    self.inner.push(value)
  }

  /// Pops a value from the stack
  ///
  /// # Returns
  ///
  /// Returns `Some(value)` if stack is not empty, `None` if empty
  ///
  /// # Examples
  ///
  /// ```
  /// use nexus_utils_embedded_rs::RcStack;
  ///
  /// let stack = RcStack::new();
  /// stack.push(1).unwrap();
  /// assert_eq!(stack.pop(), Some(1));
  /// assert_eq!(stack.pop(), None);
  /// ```
  pub fn pop(&self) -> Option<T> {
    self.inner.pop()
  }

  /// Peeks at the top value of the stack without removing it
  ///
  /// # Returns
  ///
  /// Returns `Some(value)` if stack is not empty, `None` if empty
  ///
  /// # Examples
  ///
  /// ```
  /// use nexus_utils_embedded_rs::RcStack;
  ///
  /// let stack = RcStack::new();
  /// stack.push(1).unwrap();
  /// assert_eq!(stack.peek(), Some(1));
  /// assert_eq!(stack.len().to_usize(), 1); // Element still remains
  /// ```
  pub fn peek(&self) -> Option<T>
  where
    T: Clone,
  {
    self.inner.peek()
  }

  /// Removes all elements from the stack
  ///
  /// # Examples
  ///
  /// ```
  /// use nexus_utils_embedded_rs::RcStack;
  ///
  /// let stack = RcStack::new();
  /// stack.push(1).unwrap();
  /// stack.push(2).unwrap();
  /// stack.clear();
  /// assert_eq!(stack.len().to_usize(), 0);
  /// ```
  pub fn clear(&self) {
    self.inner.clear();
  }

  /// Returns the number of elements in the stack
  ///
  /// # Returns
  ///
  /// `QueueSize` representing the current number of elements
  ///
  /// # Examples
  ///
  /// ```
  /// use nexus_utils_embedded_rs::RcStack;
  ///
  /// let stack = RcStack::new();
  /// stack.push(1).unwrap();
  /// assert_eq!(stack.len().to_usize(), 1);
  /// ```
  pub fn len(&self) -> QueueSize {
    self.inner.len()
  }

  /// Returns the capacity of the stack
  ///
  /// # Returns
  ///
  /// Limit value if capacity limit is set, `QueueSize::Limitless` if not set
  ///
  /// # Examples
  ///
  /// ```
  /// use nexus_utils_embedded_rs::RcStack;
  ///
  /// let stack: RcStack<i32> = RcStack::with_capacity(10);
  /// assert_eq!(stack.capacity().to_usize(), 10);
  /// ```
  pub fn capacity(&self) -> QueueSize {
    self.inner.capacity()
  }
}

impl<T> Default for RcStack<T> {
  fn default() -> Self {
    Self::new()
  }
}

impl<T> StackBase<T> for RcStack<T> {
  fn len(&self) -> QueueSize {
    self.inner.len()
  }

  fn capacity(&self) -> QueueSize {
    self.inner.capacity()
  }
}

impl<T> StackMut<T> for RcStack<T> {
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
    T: Clone,
  {
    self.inner.peek()
  }
}

impl<T> StackStorage<T> for RcShared<RefCell<StackBuffer<T>>> {
  fn with_read<R>(&self, f: impl FnOnce(&StackBuffer<T>) -> R) -> R {
    f(&self.borrow())
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut StackBuffer<T>) -> R) -> R {
    f(&mut self.borrow_mut())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn rc_stack_push_pop() {
    let stack = RcStack::with_capacity(1);
    stack.push(1).unwrap();
    assert!(stack.push(2).is_err());
    assert_eq!(stack.pop(), Some(1));
    assert!(stack.pop().is_none());
  }

  #[test]
  fn rc_stack_handle_operations() {
    let stack = RcStack::new();
    stack.push(10).unwrap();
    let cloned = stack.clone();
    cloned.push(11).unwrap();

    assert_eq!(stack.len().to_usize(), 2);
    assert_eq!(cloned.pop(), Some(11));
    assert_eq!(stack.pop(), Some(10));
  }

  #[test]
  fn rc_stack_peek_ref() {
    let stack = RcStack::new();
    stack.push(5).unwrap();
    assert_eq!(stack.peek(), Some(5));
    stack.pop();
    assert_eq!(stack.peek(), None);
  }
}
