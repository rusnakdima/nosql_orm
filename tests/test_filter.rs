use nosql_orm::query::Filter;
use serde_json::json;

#[test]
fn test_filter_eq() {
  let filter = Filter::Eq("name".to_string(), json!("Alice"));
  assert!(filter.matches(&json!({"name": "Alice"})));
  assert!(!filter.matches(&json!({"name": "Bob"})));
}

#[test]
fn test_filter_ne() {
  let filter = Filter::Ne("name".to_string(), json!("Alice"));
  assert!(filter.matches(&json!({"name": "Bob"})));
  assert!(!filter.matches(&json!({"name": "Alice"})));
}

#[test]
fn test_filter_gt_number() {
  let filter = Filter::Gt("age".to_string(), json!(18));
  assert!(filter.matches(&json!({"age": 25})));
  assert!(!filter.matches(&json!({"age": 10})));
}

#[test]
fn test_filter_gt_string() {
  let filter = Filter::Gt("name".to_string(), json!("C"));
  assert!(filter.matches(&json!({"name": "David"})));
  assert!(!filter.matches(&json!({"name": "Alice"})));
}

#[test]
fn test_filter_gte() {
  let filter = Filter::Gte("age".to_string(), json!(18));
  assert!(filter.matches(&json!({"age": 18})));
  assert!(filter.matches(&json!({"age": 25})));
  assert!(!filter.matches(&json!({"age": 17})));
}

#[test]
fn test_filter_lt() {
  let filter = Filter::Lt("age".to_string(), json!(65));
  assert!(filter.matches(&json!({"age": 30})));
  assert!(!filter.matches(&json!({"age": 70})));
}

#[test]
fn test_filter_lte() {
  let filter = Filter::Lte("age".to_string(), json!(65));
  assert!(filter.matches(&json!({"age": 65})));
  assert!(filter.matches(&json!({"age": 30})));
  assert!(!filter.matches(&json!({"age": 66})));
}

#[test]
fn test_filter_in() {
  let filter = Filter::In(
    "status".to_string(),
    vec![json!("active"), json!("pending")],
  );
  assert!(filter.matches(&json!({"status": "active"})));
  assert!(filter.matches(&json!({"status": "pending"})));
  assert!(!filter.matches(&json!({"status": "deleted"})));
}

#[test]
fn test_filter_not_in() {
  let filter = Filter::NotIn(
    "status".to_string(),
    vec![json!("deleted"), json!("archived")],
  );
  assert!(filter.matches(&json!({"status": "active"})));
  assert!(!filter.matches(&json!({"status": "deleted"})));
}

#[test]
fn test_filter_contains_case_insensitive() {
  let filter = Filter::Contains("name".to_string(), "ali".to_string());
  assert!(filter.matches(&json!({"name": "Alice"})));
  assert!(filter.matches(&json!({"name": "ALICE"})));
  assert!(filter.matches(&json!({"name": "malicious"})));
  assert!(!filter.matches(&json!({"name": "Bob"})));
}

#[test]
fn test_filter_starts_with() {
  let filter = Filter::StartsWith("email".to_string(), "admin".to_string());
  assert!(filter.matches(&json!({"email": "admin@example.com"})));
  assert!(filter.matches(&json!({"email": "ADMIN@example.com"})));
  assert!(!filter.matches(&json!({"email": "user@example.com"})));
}

#[test]
fn test_filter_ends_with() {
  let filter = Filter::EndsWith("email".to_string(), ".com".to_string());
  assert!(filter.matches(&json!({"email": "user@example.com"})));
  assert!(!filter.matches(&json!({"email": "user@example.org"})));
}

#[test]
fn test_filter_like_exact_match() {
  let filter = Filter::Like("name".to_string(), "Alice".to_string());
  assert!(filter.matches(&json!({"name": "Alice"})));
  assert!(!filter.matches(&json!({"name": "Bob"})));
}

#[test]
fn test_filter_like_wildcard_start() {
  let filter = Filter::Like("name".to_string(), "%ice".to_string());
  assert!(filter.matches(&json!({"name": "Alice"})));
  assert!(filter.matches(&json!({"name": "Nice"})));
  assert!(!filter.matches(&json!({"name": "Bob"})));
}

#[test]
fn test_filter_like_wildcard_end() {
  let filter = Filter::Like("name".to_string(), "Ali%".to_string());
  assert!(filter.matches(&json!({"name": "Alice"})));
  assert!(filter.matches(&json!({"name": "Alicia"})));
  assert!(!filter.matches(&json!({"name": "Bob"})));
}

#[test]
fn test_filter_like_wildcard_both() {
  let filter = Filter::Like("name".to_string(), "%lic%".to_string());
  assert!(filter.matches(&json!({"name": "Alice"})));
  assert!(filter.matches(&json!({"name": "olic"})));
}

#[test]
fn test_filter_is_null() {
  let filter = Filter::IsNull("email".to_string());
  assert!(filter.matches(&json!({"email": null})));
  assert!(!filter.matches(&json!({"email": "user@example.com"})));
}

#[test]
fn test_filter_is_not_null() {
  let filter = Filter::IsNotNull("email".to_string());
  assert!(!filter.matches(&json!({"email": null})));
  assert!(filter.matches(&json!({"email": "user@example.com"})));
}

#[test]
fn test_filter_between() {
  let filter = Filter::Between("age".to_string(), json!(18), json!(65));
  assert!(filter.matches(&json!({"age": 18})));
  assert!(filter.matches(&json!({"age": 25})));
  assert!(filter.matches(&json!({"age": 65})));
  assert!(!filter.matches(&json!({"age": 17})));
  assert!(!filter.matches(&json!({"age": 66})));
}

#[test]
fn test_filter_between_strings() {
  let filter = Filter::Between("name".to_string(), json!("a"), json!("m"));
  assert!(filter.matches(&json!({"name": "b"})));
  assert!(filter.matches(&json!({"name": "k"})));
  assert!(!filter.matches(&json!({"name": "z"})));
}

#[test]
fn test_filter_and() {
  let filter = Filter::And(vec![
    Filter::Eq("name".to_string(), json!("Alice")),
    Filter::Gte("age".to_string(), json!(18)),
  ]);
  assert!(filter.matches(&json!({"name": "Alice", "age": 25})));
  assert!(!filter.matches(&json!({"name": "Alice", "age": 15})));
  assert!(!filter.matches(&json!({"name": "Bob", "age": 25})));
}

#[test]
fn test_filter_or() {
  let filter = Filter::Or(vec![
    Filter::Eq("name".to_string(), json!("Alice")),
    Filter::Eq("name".to_string(), json!("Bob")),
  ]);
  assert!(filter.matches(&json!({"name": "Alice"})));
  assert!(filter.matches(&json!({"name": "Bob"})));
  assert!(!filter.matches(&json!({"name": "Charlie"})));
}

#[test]
fn test_filter_not() {
  let filter = Filter::Not(Box::new(Filter::Eq("status".to_string(), json!("deleted"))));
  assert!(filter.matches(&json!({"status": "active"})));
  assert!(!filter.matches(&json!({"status": "deleted"})));
}

#[test]
fn test_filter_nested() {
  let filter = Filter::And(vec![
    Filter::Or(vec![
      Filter::Eq("type".to_string(), json!("a")),
      Filter::Eq("type".to_string(), json!("b")),
    ]),
    Filter::Gte("score".to_string(), json!(50)),
  ]);
  assert!(filter.matches(&json!({"type": "a", "score": 75})));
  assert!(filter.matches(&json!({"type": "b", "score": 50})));
  assert!(!filter.matches(&json!({"type": "a", "score": 25})));
  assert!(!filter.matches(&json!({"type": "c", "score": 75})));
}

#[test]
fn test_filter_dot_notation() {
  let doc = json!({
      "address": {
          "city": "New York"
      }
  });
  let filter = Filter::Eq("address.city".to_string(), json!("New York"));
  assert!(filter.matches(&doc));
}

#[test]
fn test_filter_missing_field() {
  let filter = Filter::Eq("name".to_string(), json!("Alice"));
  assert!(!filter.matches(&json!({})));
  assert!(!filter.matches(&json!({"other": "value"})));
}

#[test]
fn test_filter_number_comparison_edge_cases() {
  assert!(Filter::Gt("n".to_string(), json!(0)).matches(&json!({"n": 0.00001})));
  assert!(Filter::Lt("n".to_string(), json!(0)).matches(&json!({"n": -0.00001})));
  assert!(Filter::Gte("n".to_string(), json!(0)).matches(&json!({"n": 0})));
  assert!(Filter::Lte("n".to_string(), json!(0)).matches(&json!({"n": 0})));
}

#[test]
fn test_filter_empty_in_list() {
  let filter = Filter::In("name".to_string(), vec![]);
  assert!(!filter.matches(&json!({"name": "anything"})));
}

#[test]
fn test_filter_like_single_char_wildcard() {
  let filter = Filter::Like("code".to_string(), "A%".to_string());
  assert!(filter.matches(&json!({"code": "ABC"})));
  assert!(filter.matches(&json!({"code": "AB"})));
  assert!(!filter.matches(&json!({"code": "BC"})));
}

#[test]
fn test_filter_complex_combination() {
  let filter = Filter::And(vec![
    Filter::Or(vec![
      Filter::Eq("role".to_string(), json!("admin")),
      Filter::Eq("role".to_string(), json!("manager")),
    ]),
    Filter::Not(Box::new(Filter::Eq(
      "status".to_string(),
      json!("suspended"),
    ))),
    Filter::Gte("access_level".to_string(), json!(5)),
  ]);

  assert!(filter.matches(&json!({
      "role": "admin",
      "status": "active",
      "access_level": 10
  })));
  assert!(filter.matches(&json!({
      "role": "manager",
      "status": "active",
      "access_level": 5
  })));
  assert!(!filter.matches(&json!({
      "role": "admin",
      "status": "suspended",
      "access_level": 10
  })));
  assert!(!filter.matches(&json!({
      "role": "user",
      "status": "active",
      "access_level": 10
  })));
}
