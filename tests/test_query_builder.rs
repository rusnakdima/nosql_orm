use nosql_orm::prelude::*;

#[test]
fn test_query_builder_basic() {
  let qb = QueryBuilder::new()
    .where_eq("name", "Alice")
    .where_gte("age", 18);

  let filter = qb.build_filter().unwrap();
  let doc = serde_json::json!({"name": "Alice", "age": 25});
  assert!(filter.matches(&doc));

  let doc2 = serde_json::json!({"name": "Alice", "age": 15});
  assert!(!filter.matches(&doc2));
}

#[test]
fn test_query_builder_with_limit_skip() {
  let qb = QueryBuilder::new()
    .where_eq("status", "active")
    .limit(10)
    .skip(5);

  let filter = qb.build_filter().unwrap();
  assert!(filter.matches(&serde_json::json!({"status": "active"})));
}

#[test]
fn test_query_builder_order_by() {
  let qb = QueryBuilder::new()
    .where_eq("active", true)
    .order_by(OrderBy::desc("created_at"));

  assert!(qb.build_filter().is_some());
}

#[test]
fn test_query_builder_or() {
  let qb1 = QueryBuilder::new().where_eq("type", "a");
  let qb2 = QueryBuilder::new().where_eq("type", "b");
  let combined = qb1.or(qb2);

  let filter = combined.build_filter().unwrap();
  assert!(filter.matches(&serde_json::json!({"type": "a"})));
  assert!(filter.matches(&serde_json::json!({"type": "b"})));
  assert!(!filter.matches(&serde_json::json!({"type": "c"})));
}

#[test]
fn test_query_builder_not() {
  let qb = QueryBuilder::new().where_eq("status", "active").not();

  let filter = qb.build_filter().unwrap();
  assert!(filter.matches(&serde_json::json!({"status": "deleted"})));
  assert!(!filter.matches(&serde_json::json!({"status": "active"})));
}

#[test]
fn test_query_builder_where_in() {
  let qb = QueryBuilder::new().where_in(
    "status",
    vec![serde_json::json!("active"), serde_json::json!("pending")],
  );

  let filter = qb.build_filter().unwrap();
  assert!(filter.matches(&serde_json::json!({"status": "active"})));
  assert!(filter.matches(&serde_json::json!({"status": "pending"})));
}

#[test]
fn test_query_builder_where_not_in() {
  let qb = QueryBuilder::new().where_not_in(
    "status",
    vec![serde_json::json!("deleted"), serde_json::json!("archived")],
  );

  let filter = qb.build_filter().unwrap();
  assert!(filter.matches(&serde_json::json!({"status": "active"})));
  assert!(!filter.matches(&serde_json::json!({"status": "deleted"})));
}

#[test]
fn test_query_builder_where_contains() {
  let qb = QueryBuilder::new().where_contains("email", "admin");

  let filter = qb.build_filter().unwrap();
  assert!(filter.matches(&serde_json::json!({"email": "admin@example.com"})));
  assert!(!filter.matches(&serde_json::json!({"email": "user@example.com"})));
}

#[test]
fn test_query_builder_where_starts_with() {
  let qb = QueryBuilder::new().where_starts_with("name", "Al");

  let filter = qb.build_filter().unwrap();
  assert!(filter.matches(&serde_json::json!({"name": "Alice"})));
  assert!(!filter.matches(&serde_json::json!({"name": "Bob"})));
}

#[test]
fn test_query_builder_where_ends_with() {
  let qb = QueryBuilder::new().where_ends_with("email", ".com");

  let filter = qb.build_filter().unwrap();
  assert!(filter.matches(&serde_json::json!({"email": "user@example.com"})));
  assert!(!filter.matches(&serde_json::json!({"email": "user@example.org"})));
}

#[test]
fn test_query_builder_where_like() {
  let qb = QueryBuilder::new().where_like("name", "%ice%");

  let filter = qb.build_filter().unwrap();
  assert!(filter.matches(&serde_json::json!({"name": "Alice"})));
  assert!(!filter.matches(&serde_json::json!({"name": "Bob"})));
}

#[test]
fn test_query_builder_where_between() {
  let qb = QueryBuilder::new().where_between("age", 18, 65);

  let filter = qb.build_filter().unwrap();
  assert!(filter.matches(&serde_json::json!({"age": 30})));
  assert!(!filter.matches(&serde_json::json!({"age": 17})));
}

#[test]
fn test_query_builder_where_is_null() {
  let qb = QueryBuilder::new().where_is_null("deleted_at");

  let filter = qb.build_filter().unwrap();
  assert!(filter.matches(&serde_json::json!({"deleted_at": null})));
  assert!(!filter.matches(&serde_json::json!({"deleted_at": "2024-01-01"})));
}

#[test]
fn test_query_builder_where_is_not_null() {
  let qb = QueryBuilder::new().where_is_not_null("email");

  let filter = qb.build_filter().unwrap();
  assert!(!filter.matches(&serde_json::json!({"email": null})));
  assert!(filter.matches(&serde_json::json!({"email": "user@example.com"})));
}

#[test]
fn test_query_builder_select_fields() {
  let qb = QueryBuilder::new().select(&["id", "name"]);

  assert!(qb.get_projection().is_some());
}

#[test]
fn test_query_builder_exclude_fields() {
  let qb = QueryBuilder::new().exclude(&["password", "token"]);

  assert!(qb.get_projection().is_some());
}

#[test]
fn test_query_builder_with_relation() {
  let qb = QueryBuilder::new()
    .where_eq("user_id", "123")
    .with_relation("posts")
    .with_relation("comments");

  let filter = qb.build_filter().unwrap();
  assert!(filter.matches(&serde_json::json!({"user_id": "123"})));
}

#[test]
fn test_query_builder_empty() {
  let qb = QueryBuilder::new();
  assert!(qb.build_filter().is_none());
}

#[test]
fn test_query_builder_multiple_filters() {
  let qb = QueryBuilder::new()
    .where_eq("status", "active")
    .where_gte("age", 18)
    .where_ne("name", "Admin")
    .where_like("email", "%@example.com");

  let filter = qb.build_filter().unwrap();
  assert!(filter.matches(&serde_json::json!({
      "status": "active",
      "age": 25,
      "name": "User",
      "email": "user@example.com"
  })));
}

#[test]
fn test_query_builder_build_filter_single() {
  let qb = QueryBuilder::new().where_eq("id", "123");
  let filter = qb.build_filter().unwrap();

  assert!(filter.matches(&serde_json::json!({"id": "123"})));
  assert!(!filter.matches(&serde_json::json!({"id": "456"})));
}

#[test]
fn test_query_builder_where_gt() {
  let qb = QueryBuilder::new().where_gt("age", 18);
  let filter = qb.build_filter().unwrap();
  assert!(filter.matches(&serde_json::json!({"age": 25})));
  assert!(!filter.matches(&serde_json::json!({"age": 10})));
}

#[test]
fn test_query_builder_where_lt() {
  let qb = QueryBuilder::new().where_lt("age", 65);
  let filter = qb.build_filter().unwrap();
  assert!(filter.matches(&serde_json::json!({"age": 30})));
  assert!(!filter.matches(&serde_json::json!({"age": 70})));
}

#[test]
fn test_query_builder_chain_all_methods() {
  let qb = QueryBuilder::new()
    .where_eq("status", "active")
    .where_gt("score", 50)
    .where_lt("age", 100)
    .where_contains("name", "test")
    .where_starts_with("email", "user")
    .where_ends_with("email", ".com")
    .where_in(
      "role",
      vec![serde_json::json!("admin"), serde_json::json!("user")],
    )
    .where_is_null("deleted_at")
    .where_like("name", "%test%")
    .limit(100)
    .skip(10)
    .order_by(OrderBy::asc("created_at"));

  let filter = qb.build_filter().unwrap();
  assert!(filter.matches(&serde_json::json!({
      "status": "active",
      "score": 75,
      "age": 30,
      "name": "test user",
      "email": "user@test.com",
      "role": "admin",
      "deleted_at": null,
      "created_at": "2024-01-01"
  })));
}
