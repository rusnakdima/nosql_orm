use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
  Asc,
  Desc,
}

#[derive(Debug, Clone)]
pub struct Cursor {
  pub last_id: String,
  pub sort_field: String,
  pub sort_asc: bool,
}

impl Cursor {
  pub fn new(last_id: String, sort_field: String, sort_asc: bool) -> Self {
    Self {
      last_id,
      sort_field,
      sort_asc,
    }
  }

  pub fn as_filter(&self) -> super::Filter {
    if self.sort_asc {
      super::Filter::Gt(self.sort_field.clone(), Value::String(self.last_id.clone()))
    } else {
      super::Filter::Lt(self.sort_field.clone(), Value::String(self.last_id.clone()))
    }
  }
}

impl Default for Cursor {
  fn default() -> Self {
    Self {
      last_id: String::new(),
      sort_field: String::new(),
      sort_asc: true,
    }
  }
}

#[derive(Debug)]
pub struct PaginatedResult<T> {
  pub data: Vec<T>,
  pub next_cursor: Option<Cursor>,
  pub has_more: bool,
}

#[derive(Debug, Clone)]
pub struct OrderBy {
  pub field: String,
  pub direction: SortDirection,
}

impl OrderBy {
  pub fn asc(field: impl Into<String>) -> Self {
    Self {
      field: field.into(),
      direction: SortDirection::Asc,
    }
  }
  pub fn desc(field: impl Into<String>) -> Self {
    Self {
      field: field.into(),
      direction: SortDirection::Desc,
    }
  }
}
