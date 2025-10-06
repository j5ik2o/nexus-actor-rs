#[derive(Debug, Clone)]
pub struct Stack<T> {
  items: Vec<T>,
}

impl<T> Stack<T> {
  pub fn new() -> Self {
    Stack { items: Vec::new() }
  }

  pub fn push(&mut self, item: T) {
    self.items.push(item);
  }

  pub fn pop(&mut self) -> Option<T> {
    self.items.pop()
  }

  pub fn peek(&self) -> Option<T>
  where
    T: Clone, {
    self.items.last().cloned()
  }

  pub fn is_empty(&self) -> bool {
    self.items.is_empty()
  }

  pub fn size(&self) -> usize {
    self.items.len()
  }

  pub fn clear(&mut self) {
    self.items.clear();
  }
}

impl<T> Default for Stack<T> {
  fn default() -> Self {
    Stack::new()
  }
}
