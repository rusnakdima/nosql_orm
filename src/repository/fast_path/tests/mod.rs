use super::*;

#[tokio::test]
async fn test_simple_filter_check() {
  let eq_filter = crate::query::Filter::Eq("id".to_string(), serde_json::json!("123"));
  assert!(FastQueryExecutor::is_simple_filter(&eq_filter));

  let and_filter = crate::query::Filter::And(vec![eq_filter.clone()]);
  assert!(!FastQueryExecutor::is_simple_filter(&and_filter));
}
