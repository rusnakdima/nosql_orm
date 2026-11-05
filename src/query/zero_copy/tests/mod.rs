use super::*;

#[test]
fn test_direct_deserializer() {
  let str_value = Value::String("hello".to_string());
  assert_eq!(
    DirectDeserializer::deserialize_string(&str_value),
    Some("hello".to_string())
  );

  let num_value = Value::Number(serde_json::Number::from(42));
  assert_eq!(DirectDeserializer::deserialize_i64(&num_value), Some(42));

  let bool_value = Value::Bool(true);
  assert_eq!(
    DirectDeserializer::deserialize_bool(&bool_value),
    Some(true)
  );
}

#[test]
fn test_lazy_value() {
  let value = Value::String("test".to_string());
  let lazy = LazyValue::new(&value);
  assert_eq!(lazy.as_str(), Some("test"));
  assert!(!lazy.is_null());
}
