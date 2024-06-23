use std::path::Path;

use backtrace::Backtrace;

#[derive(Debug, Clone)]
pub struct CallerInfo {
  pub(crate) fname: String,
  pub(crate) line: u32,
}

impl CallerInfo {
  pub fn new(skip: usize) -> Self {
    let bt = Backtrace::new();
    let frames = bt.frames();

    if skip + 1 >= frames.len() {
      return CallerInfo {
        fname: String::new(),
        line: 0,
      };
    }

    let frame = &frames[skip + 1]; // +1 to skip this function itself
    let symbol = frame.symbols().get(0);

    if let Some(symbol) = symbol {
      let file = symbol
        .filename()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
      let line = symbol.lineno().unwrap_or(0);
      Self { fname: file, line }
    } else {
      Self {
        fname: String::new(),
        line: 0,
      }
    }
  }

  pub fn short_file_name(&self) -> String {
    Path::new(&self.fname)
      .file_name()
      .and_then(|name| name.to_str())
      .map(String::from)
      .unwrap_or_else(|| self.fname.clone())
  }
}

impl std::fmt::Display for CallerInfo {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}:{}", self.short_file_name(), self.line)
  }
}
