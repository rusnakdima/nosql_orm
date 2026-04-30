use super::{ValidationError, ValidationResult};
use serde_json::Value;

pub trait FieldValidator: Send + Sync {
  fn validate(&self, field: &str, value: &Value) -> Result<(), ValidationError>;
}

type ValidateFn<E> = dyn Fn(&E) -> ValidationResult + Send + Sync;

macro_rules! impl_range_validator {
  ($name:ident, $type:ty, $as_type:ident, $len_expr:expr, $value_get:expr, $min_msg:expr, $max_msg:expr) => {
    pub struct $name {
      pub min: Option<$type>,
      pub max: Option<$type>,
    }
    impl Default for $name {
      fn default() -> Self {
        Self::new()
      }
    }
    impl $name {
      pub fn new() -> Self {
        Self {
          min: None,
          max: None,
        }
      }
      pub fn min(mut self, min: $type) -> Self {
        self.min = Some(min);
        self
      }
      pub fn max(mut self, max: $type) -> Self {
        self.max = Some(max);
        self
      }
    }
    impl FieldValidator for $name {
      fn validate(&self, field: &str, value: &Value) -> Result<(), ValidationError> {
        let x = value
          .$as_type()
          .ok_or_else(|| ValidationError::field(field, $value_get))?;
        let len = $len_expr(x);
        if let Some(min) = self.min {
          if len < min {
            return Err(ValidationError::field(field, format!($min_msg, min)));
          }
        }
        if let Some(max) = self.max {
          if len > max {
            return Err(ValidationError::field(field, format!($max_msg, max)));
          }
        }
        Ok(())
      }
    }
  };
}

impl_range_validator!(
  LengthValidator,
  usize,
  as_str,
  |s: &str| s.len(),
  "Expected string",
  "Minimum length is {}",
  "Maximum length is {}"
);

impl_range_validator!(
  RangeValidator,
  f64,
  as_f64,
  |x: f64| x,
  "Expected number",
  "Minimum value is {}",
  "Maximum value is {}"
);

pub struct PatternValidator {
  pub pattern: regex::Regex,
}

impl PatternValidator {
  pub fn new(pattern: &str) -> Result<Self, regex::Error> {
    Ok(Self {
      pattern: regex::Regex::new(pattern)?,
    })
  }
}

impl FieldValidator for PatternValidator {
  fn validate(&self, field: &str, value: &Value) -> Result<(), ValidationError> {
    let s = value
      .as_str()
      .ok_or_else(|| ValidationError::field(field, "Expected string"))?;
    if !self.pattern.is_match(s) {
      return Err(ValidationError::field(field, "Pattern mismatch"));
    }
    Ok(())
  }
}

pub struct EnumValidator {
  pub allowed: Vec<Value>,
}

impl EnumValidator {
  pub fn new(allowed: Vec<Value>) -> Self {
    Self { allowed }
  }
}

impl FieldValidator for EnumValidator {
  fn validate(&self, field: &str, value: &Value) -> Result<(), ValidationError> {
    if !self.allowed.contains(value) {
      return Err(ValidationError::field(field, "Value not in allowed list"));
    }
    Ok(())
  }
}

pub struct EmailValidator;

impl FieldValidator for EmailValidator {
  fn validate(&self, field: &str, value: &Value) -> Result<(), ValidationError> {
    let s = value
      .as_str()
      .ok_or_else(|| ValidationError::field(field, "Expected string"))?;
    if !s.contains('@') || !s.contains('.') {
      return Err(ValidationError::field(field, "Invalid email format"));
    }
    Ok(())
  }
}

pub struct CompositeValidator {
  pub validators: Vec<Box<dyn FieldValidator>>,
}

impl Default for CompositeValidator {
  fn default() -> Self {
    Self::new()
  }
}

impl CompositeValidator {
  pub fn new() -> Self {
    Self {
      validators: Vec::new(),
    }
  }
  pub fn add_validator<V: FieldValidator + 'static>(mut self, v: V) -> Self {
    self.validators.push(Box::new(v));
    self
  }
}

impl FieldValidator for CompositeValidator {
  fn validate(&self, field: &str, value: &Value) -> Result<(), ValidationError> {
    for v in &self.validators {
      v.validate(field, value)?;
    }
    Ok(())
  }
}

pub struct EntityValidator<E> {
  pub fields: std::collections::HashMap<String, Box<dyn FieldValidator>>,
  pub validate_fn: Option<Box<ValidateFn<E>>>,
}

impl<E: serde::Serialize> Default for EntityValidator<E> {
  fn default() -> Self {
    Self::new()
  }
}

impl<E: serde::Serialize> EntityValidator<E> {
  pub fn new() -> Self {
    Self {
      fields: std::collections::HashMap::new(),
      validate_fn: None,
    }
  }

  pub fn add_field(mut self, field: &str, validator: impl FieldValidator + 'static) -> Self {
    self.fields.insert(field.to_string(), Box::new(validator));
    self
  }

  pub fn with_validate(
    mut self,
    f: impl Fn(&E) -> ValidationResult + Send + Sync + 'static,
  ) -> Self {
    self.validate_fn = Some(Box::new(f));
    self
  }

  pub fn validate(&self, entity: &E) -> ValidationResult {
    let json = serde_json::to_value(entity).map_err(|e| ValidationError::general(e.to_string()))?;
    for (field, validator) in &self.fields {
      if let Some(value) = json.get(field) {
        validator.validate(field, value)?;
      }
    }
    if let Some(ref f) = self.validate_fn {
      f(entity)?;
    }
    Ok(())
  }
}
