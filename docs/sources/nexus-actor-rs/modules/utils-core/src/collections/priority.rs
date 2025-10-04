use super::Element;

pub const PRIORITY_LEVELS: usize = 8;
pub const DEFAULT_PRIORITY: i8 = (PRIORITY_LEVELS / 2) as i8;

pub trait PriorityMessage: Element {
  fn get_priority(&self) -> Option<i8>;
}
