pub mod actor;
pub mod ctxext;
pub mod event_stream;
#[cfg(test)]
mod event_stream_test;
pub mod log;
pub mod util;

pub fn add(left: usize, right: usize) -> usize {
  left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
  fn it_works() {
    let result = add(2, 2);
    assert_eq!(result, 4);
  }
}
