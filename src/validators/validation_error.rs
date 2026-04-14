#[derive(Debug, Clone, thiserror::Error)]
pub enum ValidationError {
  #[error("Field '{field}': {message}")]
  Field { field: String, message: String },

  #[error("Validation failed: {0}")]
  General(String),
}

pub type ValidationResult<T = ()> = Result<T, ValidationError>;

impl ValidationError {
  pub fn field(field: impl Into<String>, message: impl Into<String>) -> Self {
    Self::Field {
      field: field.into(),
      message: message.into(),
    }
  }

  pub fn general(message: impl Into<String>) -> Self {
    Self::General(message.into())
  }
}
