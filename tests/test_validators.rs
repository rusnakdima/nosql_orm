use nosql_orm::validators::{
  EmailValidator, EnumValidator, FieldValidator, LengthValidator, PatternValidator, RangeValidator,
};
use serde_json::json;

#[test]
fn test_length_validator_min() {
  let validator = LengthValidator::new().min(5);
  let result = validator.validate("field", &json!("hello"));
  assert!(result.is_ok());

  let result = validator.validate("field", &json!("hi"));
  assert!(result.is_err());
}

#[test]
fn test_length_validator_max() {
  let validator = LengthValidator::new().max(10);
  let result = validator.validate("field", &json!("hello"));
  assert!(result.is_ok());

  let result = validator.validate("field", &json!("this is too long"));
  assert!(result.is_err());
}

#[test]
fn test_length_validator_min_max() {
  let validator = LengthValidator::new().min(3).max(10);
  assert!(validator.validate("field", &json!("hello")).is_ok());
  assert!(validator.validate("field", &json!("hi")).is_err());
  assert!(validator
    .validate("field", &json!("this is too long"))
    .is_err());
}

#[test]
fn test_length_validator_non_string() {
  let validator = LengthValidator::new().min(5);
  let result = validator.validate("field", &json!(123));
  assert!(result.is_err());
}

#[test]
fn test_range_validator_min() {
  let validator = RangeValidator::new().min(18.0);
  assert!(validator.validate("field", &json!(25.0)).is_ok());
  assert!(validator.validate("field", &json!(15.0)).is_err());
}

#[test]
fn test_range_validator_max() {
  let validator = RangeValidator::new().max(65.0);
  assert!(validator.validate("field", &json!(30.0)).is_ok());
  assert!(validator.validate("field", &json!(70.0)).is_err());
}

#[test]
fn test_range_validator_min_max() {
  let validator = RangeValidator::new().min(18.0).max(65.0);
  assert!(validator.validate("field", &json!(30.0)).is_ok());
  assert!(validator.validate("field", &json!(15.0)).is_err());
  assert!(validator.validate("field", &json!(70.0)).is_err());
}

#[test]
fn test_range_validator_non_number() {
  let validator = RangeValidator::new().min(5.0);
  let result = validator.validate("field", &json!("not a number"));
  assert!(result.is_err());
}

#[test]
fn test_pattern_validator_match() {
  let validator = PatternValidator::new(r"^\d{3}-\d{2}-\d{4}$").unwrap();
  assert!(validator.validate("field", &json!("123-45-6789")).is_ok());
  assert!(validator.validate("field", &json!("abc-def-ghij")).is_err());
}

#[test]
fn test_pattern_validator_email() {
  let validator =
    PatternValidator::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap();
  assert!(validator
    .validate("field", &json!("user@example.com"))
    .is_ok());
  assert!(validator
    .validate("field", &json!("invalid-email"))
    .is_err());
}

#[test]
fn test_pattern_validator_invalid_regex() {
  let result = PatternValidator::new(r"[invalid");
  assert!(result.is_err());
}

#[test]
fn test_enum_validator_allowed() {
  let validator = EnumValidator::new(vec![json!("active"), json!("pending"), json!("draft")]);
  assert!(validator.validate("field", &json!("active")).is_ok());
  assert!(validator.validate("field", &json!("pending")).is_ok());
  assert!(validator.validate("field", &json!("draft")).is_ok());
}

#[test]
fn test_enum_validator_not_allowed() {
  let validator = EnumValidator::new(vec![json!("active"), json!("pending")]);
  let result = validator.validate("field", &json!("deleted"));
  assert!(result.is_err());
}

#[test]
fn test_enum_validator_numbers() {
  let validator = EnumValidator::new(vec![json!(1), json!(2), json!(3)]);
  assert!(validator.validate("field", &json!(2)).is_ok());
  assert!(validator.validate("field", &json!(5)).is_err());
}

#[test]
fn test_email_validator_valid() {
  let validator = EmailValidator;
  assert!(validator
    .validate("field", &json!("user@example.com"))
    .is_ok());
  assert!(validator
    .validate("field", &json!("test.user@domain.co.uk"))
    .is_ok());
}

#[test]
fn test_email_validator_invalid() {
  let validator = EmailValidator;
  assert!(validator.validate("field", &json!("not-an-email")).is_err());
  assert!(validator
    .validate("field", &json!("missing@domain"))
    .is_err());
  assert!(validator
    .validate("field", &json!("no-at-sign.com"))
    .is_err());
}

#[test]
fn test_email_validator_non_string() {
  let validator = EmailValidator;
  let result = validator.validate("field", &json!(123));
  assert!(result.is_err());
}

#[test]
fn test_composite_validator_all_pass() {
  use nosql_orm::validators::CompositeValidator;
  let validator = CompositeValidator::new()
    .add(LengthValidator::new().min(5).max(20))
    .add(PatternValidator::new(r"^[a-zA-Z0-9_]+$").unwrap());

  assert!(validator.validate("field", &json!("ValidUser123")).is_ok());
}

#[test]
fn test_composite_validator_one_fails() {
  use nosql_orm::validators::CompositeValidator;
  let validator = CompositeValidator::new()
    .add(LengthValidator::new().min(5).max(10))
    .add(PatternValidator::new(r"^[a-zA-Z]+$").unwrap());

  let result = validator.validate("field", &json!("User123")); // has numbers
  assert!(result.is_err());
}

#[test]
fn test_composite_validator_empty() {
  use nosql_orm::validators::CompositeValidator;
  let validator = CompositeValidator::new();
  assert!(validator.validate("field", &json!("anything")).is_ok());
}
