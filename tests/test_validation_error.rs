use nosql_orm::validators::ValidationError;

#[test]
fn test_validation_error_field() {
  let err = ValidationError::field("email", "Invalid email format");
  assert!(err.to_string().contains("email"));
  assert!(err.to_string().contains("Invalid email format"));
}

#[test]
fn test_validation_error_general() {
  let err = ValidationError::general("Something went wrong");
  assert!(err.to_string().contains("Something went wrong"));
}

#[test]
fn test_validation_error_debug() {
  let err = ValidationError::field("name", "Too short");
  let debug = format!("{:?}", err);
  assert!(debug.contains("Field"));
  assert!(debug.contains("name"));
}

#[test]
fn test_validation_error_clone() {
  let err = ValidationError::field("field", "message");
  let cloned = err.clone();
  assert_eq!(format!("{}", err), format!("{}", cloned));
}

#[test]
fn test_validation_result_ok() {
  use nosql_orm::validators::ValidationResult;
  let result: ValidationResult = Ok(());
  assert!(result.is_ok());
}

#[test]
fn test_validation_result_err() {
  use nosql_orm::validators::ValidationResult;
  let result: ValidationResult = Err(ValidationError::field("x", "y"));
  assert!(result.is_err());
}
