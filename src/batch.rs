// Batch outcome with per-row detail: a batch that does not fail wholesale
// reports which rows were written, and every failure keeps its index into
// the submitted order plus the reason.

use crate::error::OrmError;

#[derive(Debug, Default)]
pub struct BatchResult {
  pub succeeded: usize,
  pub failed: Vec<(usize, OrmError)>,
}

impl BatchResult {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn is_success(&self) -> bool {
    self.failed.is_empty()
  }

  pub fn total(&self) -> usize {
    self.succeeded + self.failed.len()
  }

  // Folds `other` — indexed within its own chunk — into a batch-wide
  // result, shifting every failure index by `index_offset`.
  pub fn absorb(&mut self, other: BatchResult, index_offset: usize) {
    self.succeeded += other.succeeded;
    for (index, err) in other.failed {
      self.failed.push((index + index_offset, err));
    }
  }

  // One-line summary for logs: "3 ok, 1 failed (row 1: ...)".
  pub fn summary(&self) -> String {
    if self.failed.is_empty() {
      return format!("{} ok, 0 failed", self.succeeded);
    }
    let first = &self.failed[0];
    format!(
      "{} ok, {} failed (first: row {}: {})",
      self.succeeded,
      self.failed.len(),
      first.0,
      first.1
    )
  }
}
