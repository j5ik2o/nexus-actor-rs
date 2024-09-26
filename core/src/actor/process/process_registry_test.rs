#[cfg(test)]
mod tests {
  use crate::actor::process::process_registry::uint64_to_id;
  use std::time::Instant;

  const ITERATIONS: u32 = 1_000_000; // 適切な反復回数に調整してください

  #[test]
  fn test_uint64_to_id() {
    let start = Instant::now();
    let mut s = String::new();
    for i in 0..ITERATIONS {
      s = uint64_to_id(u64::from(i) << 5);
    }
    let duration = start.elapsed();
    tracing::debug!("uint64_to_id: {:?}, last result: {}", duration, s);
  }
}
