use super::{ValidationError, ValidationResult};
use serde_json::Value;

pub trait FieldValidator: Send + Sync {
  fn validate(&self, field: &str, value: &Value) -> Result<(), ValidationError>;
}

pub struct LengthValidator {
  pub min: Option<usize>,
  pub max: Option<usize>,
}

impl LengthValidator {
  pub fn new() -> Self {
    Self {
      min: None,
      max: None,
    }
  }
  pub fn min(mut self, min: usize) -> Self {
    self.min = Some(min);
    self
  }
  pub fn max(mut self, max: usize) -> Self {
    self.max = Some(max);
    self
  }
}

impl FieldValidator for LengthValidator {
  fn validate(&self, field: &str, value: &Value) -> Result<(), ValidationError> {
    let s = value
      .as_str()
      .ok_or_else(|| ValidationError::field(field, "Expected string"))?;
    if let Some(min) = self.min {
      if s.len() < min {
        return Err(ValidationError::field(
          field,
          format!("Minimum length is {}", min),
        ));
      }
    }
    if let Some(max) = self.max {
      if s.len() > max {
        return Err(ValidationError::field(
          field,
          format!("Maximum length is {}", max),
        ));
      }
    }
    Ok(())
  }
}

pub struct RangeValidator {
  pub min: Option<f64>,
  pub max: Option<f64>,
}

impl RangeValidator {
  pub fn new() -> Self {
    Self {
      min: None,
      max: None,
    }
  }
  pub fn min(mut self, min: f64) -> Self {
    self.min = Some(min);
    self
  }
  pub fn max(mut self, max: f64) -> Self {
    self.max = Some(max);
    self
  }
}

impl FieldValidator for RangeValidator {
  fn validate(&self, field: &str, value: &Value) -> Result<(), ValidationError> {
    let n = value
      .as_f64()
      .ok_or_else(|| ValidationError::field(field, "Expected number"))?;
    if let Some(min) = self.min {
      if n < min {
        return Err(ValidationError::field(
          field,
          format!("Minimum value is {}", min),
        ));
      }
    }
    if let Some(max) = self.max {
      if n > max {
        return Err(ValidationError::field(
          field,
          format!("Maximum value is {}", max),
        ));
      }
    }
    Ok(())
  }
}

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

impl CompositeValidator {
  pub fn new() -> Self {
    Self {
      validators: Vec::new(),
    }
  }
  pub fn add<V: FieldValidator + 'static>(mut self, v: V) -> Self {
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
  pub validate_fn: Option<Box<dyn Fn(&E) -> ValidationResult + Send + Sync>>,
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
