use alloc::vec::Vec;

use crate::collections::{QueueError, QueueSize};

/// Error type specific to stack operations.
///
/// Represents errors that can occur during stack buffer operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StackError<T> {
  /// Error when attempting to add an element while the stack is at capacity.
  ///
  /// # Field
  ///
  /// * Contains the value that was attempted to be added.
  Full(T),
}

/// Implementation to convert `StackError` to `QueueError`.
///
/// Converts stack-specific errors to generic queue errors.
impl<T> From<StackError<T>> for QueueError<T> {
  fn from(err: StackError<T>) -> Self {
    match err {
      StackError::Full(value) => QueueError::Full(value),
    }
  }
}

/// Owned stack buffer that stores values in LIFO (Last-In-First-Out) order.
///
/// # Overview
///
/// `StackBuffer` is a data structure that manages elements in Last-In-First-Out order.
/// An optional capacity limit can be set, and adding operations return errors if the limit is exceeded.
///
/// # Examples
///
/// ```
/// use nexus_utils_core_rs::StackBuffer;
///
/// let mut stack = StackBuffer::new();
/// stack.push(1).unwrap();
/// stack.push(2).unwrap();
/// assert_eq!(stack.pop(), Some(2));
/// assert_eq!(stack.pop(), Some(1));
/// ```
#[derive(Debug, Clone)]
pub struct StackBuffer<T> {
  items: Vec<T>,
  capacity: Option<usize>,
}

impl<T> StackBuffer<T> {
  /// Creates a new empty `StackBuffer` without capacity limit.
  ///
  /// # Returns
  ///
  /// Empty stack buffer with unlimited capacity.
  ///
  /// # Examples
  ///
  /// ```
  /// use nexus_utils_core_rs::StackBuffer;
  ///
  /// let stack: StackBuffer<i32> = StackBuffer::new();
  /// assert!(stack.is_empty());
  /// ```
  pub fn new() -> Self {
    Self {
      items: Vec::new(),
      capacity: None,
    }
  }

  /// Creates a new `StackBuffer` with the specified capacity limit.
  ///
  /// # Arguments
  ///
  /// * `capacity` - Maximum capacity of the stack.
  ///
  /// # Returns
  ///
  /// Empty stack buffer with the specified capacity limit.
  ///
  /// # Examples
  ///
  /// ```
  /// use nexus_utils_core_rs::StackBuffer;
  ///
  /// let mut stack = StackBuffer::with_capacity(3);
  /// stack.push(1).unwrap();
  /// stack.push(2).unwrap();
  /// stack.push(3).unwrap();
  /// // Next push returns error because capacity limit is reached
  /// assert!(stack.push(4).is_err());
  /// ```
  pub fn with_capacity(capacity: usize) -> Self {
    Self {
      items: Vec::with_capacity(capacity),
      capacity: Some(capacity),
    }
  }

  /// Gets the stack's capacity limit.
  ///
  /// # Returns
  ///
  /// * `QueueSize::limited` if capacity limit is set.
  /// * `QueueSize::limitless` if unlimited capacity.
  ///
  /// # Examples
  ///
  /// ```
  /// use nexus_utils_core_rs::StackBuffer;
  ///
  /// let stack = StackBuffer::<i32>::with_capacity(10);
  /// assert_eq!(stack.capacity().to_usize(), 10);
  /// ```
  pub fn capacity(&self) -> QueueSize {
    match self.capacity {
      Some(limit) => QueueSize::limited(limit),
      None => QueueSize::limitless(),
    }
  }

  /// Sets the stack's capacity limit.
  ///
  /// If the new capacity is less than the current number of elements, the stack is truncated to the new capacity.
  ///
  /// # Arguments
  ///
  /// * `capacity` - New capacity limit. Unlimited capacity if `None`.
  ///
  /// # Examples
  ///
  /// ```
  /// use nexus_utils_core_rs::StackBuffer;
  ///
  /// let mut stack = StackBuffer::new();
  /// stack.push(1).unwrap();
  /// stack.push(2).unwrap();
  /// stack.push(3).unwrap();
  ///
  /// // Limiting capacity to 2 removes oldest elements
  /// stack.set_capacity(Some(2));
  /// assert_eq!(stack.len().to_usize(), 2);
  /// ```
  pub fn set_capacity(&mut self, capacity: Option<usize>) {
    self.capacity = capacity;
    if let Some(limit) = capacity {
      if self.items.len() > limit {
        self.items.truncate(limit);
      }
    }
  }

  /// Gets the current number of elements in the stack.
  ///
  /// # Returns
  ///
  /// `QueueSize` representing the current element count.
  ///
  /// # Examples
  ///
  /// ```
  /// use nexus_utils_core_rs::StackBuffer;
  ///
  /// let mut stack = StackBuffer::new();
  /// stack.push(1).unwrap();
  /// stack.push(2).unwrap();
  /// assert_eq!(stack.len().to_usize(), 2);
  /// ```
  pub fn len(&self) -> QueueSize {
    QueueSize::limited(self.items.len())
  }

  /// Checks if the stack is empty.
  ///
  /// # Returns
  ///
  /// * `true` if the stack is empty.
  /// * `false` if there are one or more elements.
  ///
  /// # Examples
  ///
  /// ```
  /// use nexus_utils_core_rs::StackBuffer;
  ///
  /// let mut stack = StackBuffer::new();
  /// assert!(stack.is_empty());
  /// stack.push(1).unwrap();
  /// assert!(!stack.is_empty());
  /// ```
  pub fn is_empty(&self) -> bool {
    self.items.is_empty()
  }

  /// Adds an element to the top of the stack.
  ///
  /// Returns an error if a capacity limit is set and the stack is already full.
  ///
  /// # Arguments
  ///
  /// * `value` - Value to add to the stack.
  ///
  /// # Returns
  ///
  /// * `Ok(())` on success.
  /// * `Err(StackError::Full(value))` if stack is full.
  ///
  /// # Examples
  ///
  /// ```
  /// use nexus_utils_core_rs::StackBuffer;
  ///
  /// let mut stack = StackBuffer::with_capacity(2);
  /// assert!(stack.push(1).is_ok());
  /// assert!(stack.push(2).is_ok());
  /// assert!(stack.push(3).is_err()); // Capacity limit
  /// ```
  pub fn push(&mut self, value: T) -> Result<(), StackError<T>> {
    if matches!(self.capacity, Some(limit) if self.items.len() >= limit) {
      return Err(StackError::Full(value));
    }
    self.items.push(value);
    Ok(())
  }

  /// Removes an element from the top of the stack.
  ///
  /// This operation removes and returns the last added element (LIFO order).
  ///
  /// # Returns
  ///
  /// * `Some(value)` if there are elements in the stack.
  /// * `None` if the stack is empty.
  ///
  /// # Examples
  ///
  /// ```
  /// use nexus_utils_core_rs::StackBuffer;
  ///
  /// let mut stack = StackBuffer::new();
  /// stack.push(1).unwrap();
  /// stack.push(2).unwrap();
  /// assert_eq!(stack.pop(), Some(2));
  /// assert_eq!(stack.pop(), Some(1));
  /// assert_eq!(stack.pop(), None);
  /// ```
  pub fn pop(&mut self) -> Option<T> {
    self.items.pop()
  }

  /// Gets a reference to the top element (without removing it).
  ///
  /// # Returns
  ///
  /// * `Some(&value)` if there are elements in the stack.
  /// * `None` if the stack is empty.
  ///
  /// # Examples
  ///
  /// ```
  /// use nexus_utils_core_rs::StackBuffer;
  ///
  /// let mut stack = StackBuffer::new();
  /// stack.push(1).unwrap();
  /// stack.push(2).unwrap();
  /// assert_eq!(stack.peek(), Some(&2));
  /// assert_eq!(stack.len().to_usize(), 2); // Element not removed
  /// ```
  pub fn peek(&self) -> Option<&T> {
    self.items.last()
  }

  /// Removes all elements from the stack.
  ///
  /// After this operation, the stack is empty.
  ///
  /// # Examples
  ///
  /// ```
  /// use nexus_utils_core_rs::StackBuffer;
  ///
  /// let mut stack = StackBuffer::new();
  /// stack.push(1).unwrap();
  /// stack.push(2).unwrap();
  /// stack.clear();
  /// assert!(stack.is_empty());
  /// ```
  pub fn clear(&mut self) {
    self.items.clear();
  }
}

/// Default implementation for `StackBuffer`.
///
/// Calls `new()` to create an empty stack buffer with unlimited capacity.
impl<T> Default for StackBuffer<T> {
  fn default() -> Self {
    Self::new()
  }
}
