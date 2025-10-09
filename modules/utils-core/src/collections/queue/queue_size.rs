/// キューのサイズ制限を表す列挙型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueSize {
  /// 制限なし（無制限）。
  Limitless,
  /// 指定されたサイズまで制限。
  Limited(usize),
}

impl QueueSize {
  /// 無制限サイズのキューを表す定数コンストラクタ。
  pub const fn limitless() -> Self {
    Self::Limitless
  }

  /// 指定されたサイズで制限されたキューを表す定数コンストラクタ。
  pub const fn limited(value: usize) -> Self {
    Self::Limited(value)
  }

  /// このサイズが無制限かどうかを判定する。
  pub const fn is_limitless(&self) -> bool {
    matches!(self, Self::Limitless)
  }

  /// サイズを`usize`として取得する。無制限の場合は`usize::MAX`を返す。
  pub const fn to_usize(self) -> usize {
    match self {
      Self::Limitless => usize::MAX,
      Self::Limited(value) => value,
    }
  }
}

impl Default for QueueSize {
  fn default() -> Self {
    QueueSize::limited(0)
  }
}

#[cfg(test)]
mod tests {
  use super::QueueSize;

  #[test]
  fn queue_size_helpers_work_as_expected() {
    let zero = QueueSize::limited(0);
    let limitless = QueueSize::limitless();

    assert!(!zero.is_limitless());
    assert_eq!(zero.to_usize(), 0);

    assert!(limitless.is_limitless());
    assert_eq!(limitless.to_usize(), usize::MAX);

    match limitless {
      QueueSize::Limitless => {}
      _ => panic!("expected limitless variant"),
    }

    match zero {
      QueueSize::Limited(value) => assert_eq!(value, 0),
      _ => panic!("expected limited variant"),
    }
  }
}
