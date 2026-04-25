use nosql_orm::error::{OrmError, OrmResult};
use std::io;

#[test]
fn test_orm_error_not_found() {
  let err = OrmError::NotFound("user/123".to_string());
  assert!(err.to_string().contains("user/123"));
  assert!(err.to_string().contains("not found"));
}

#[test]
fn test_orm_error_duplicate() {
  let err = OrmError::Duplicate("id=123".to_string());
  assert!(err.to_string().contains("id=123"));
  assert!(err.to_string().contains("Duplicate"));
}

#[test]
fn test_orm_error_serialization() {
  let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
  let err = OrmError::Serialization(json_err);
  assert!(err.to_string().contains("Serialization"));
}

#[test]
fn test_orm_error_io() {
  let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
  let err = OrmError::Io(io_err);
  assert!(err.to_string().contains("IO"));
}

#[test]
fn test_orm_error_provider() {
  let err = OrmError::Provider("Connection refused".to_string());
  assert!(err.to_string().contains("Connection refused"));
  assert!(err.to_string().contains("Provider"));
}

#[test]
fn test_orm_error_relation() {
  let err = OrmError::Relation("Unknown relation 'foo'".to_string());
  assert!(err.to_string().contains("Unknown relation"));
}

#[test]
fn test_orm_error_invalid_query() {
  let err = OrmError::InvalidQuery("Invalid filter".to_string());
  assert!(err.to_string().contains("Invalid filter"));
}

#[test]
fn test_orm_error_invalid_input() {
  let err = OrmError::InvalidInput("Empty name".to_string());
  assert!(err.to_string().contains("Empty name"));
}

#[test]
fn test_orm_error_query() {
  let err = OrmError::Query("Syntax error".to_string());
  assert!(err.to_string().contains("Syntax error"));
}

#[test]
fn test_orm_error_connection() {
  let err = OrmError::Connection("timeout".to_string());
  assert!(err.to_string().contains("timeout"));
}

#[test]
fn test_orm_error_transaction() {
  let err = OrmError::Transaction("deadlock".to_string());
  assert!(err.to_string().contains("deadlock"));
}

#[test]
fn test_orm_error_cascade_restricted() {
  let err = OrmError::CascadeRestricted {
    entity: "Post".to_string(),
    relation: "comments".to_string(),
  };
  let msg = err.to_string();
  assert!(msg.contains("Post"));
  assert!(msg.contains("comments"));
  assert!(msg.contains("cannot delete"));
}

#[test]
fn test_orm_result_ok() {
  let result: OrmResult<i32> = Ok(42);
  assert!(result.is_ok());
  assert_eq!(result.unwrap(), 42);
}

#[test]
fn test_orm_result_err() {
  let result: OrmResult<i32> = Err(OrmError::NotFound("test".to_string()));
  assert!(result.is_err());
  assert!(matches!(result.unwrap_err(), OrmError::NotFound(_)));
}

#[test]
fn test_orm_error_debug() {
  let err = OrmError::NotFound("123".to_string());
  let debug = format!("{:?}", err);
  assert!(debug.contains("NotFound"));
}
