use super::*;
use proptest::prelude::*;

proptest! {
  #[test]
  fn proptest_query_builder_chaining(field in ".+", value in ".+") {
    let query = QueryBuilder::new()
      .where_eq(&field, &*value)
      .where_gt(&field, &*value)
      .where_lt(&field, &*value);
    prop_assert!(!query.filters.is_empty());
  }

  #[test]
  fn proptest_query_builder_with_skip_limit(skip: u64, limit: u64) {
    let query = QueryBuilder::new()
      .skip(skip)
      .limit(limit);
    prop_assert_eq!(query.skip, Some(skip));
    prop_assert_eq!(query.limit, Some(limit));
  }

  #[test]
  fn proptest_query_builder_negate(field in ".+", value: u64) {
    let query = QueryBuilder::new()
      .where_eq(&field, value)
      .negate();
    prop_assert!(!query.filters.is_empty());
  }

  #[test]
  fn proptest_query_builder_or(field in ".+", value: u64) {
    let q1 = QueryBuilder::new().where_eq(&field, value);
    let q2 = QueryBuilder::new().where_ne(&field, value);
    let combined = q1.or(q2);
    prop_assert!(!combined.filters.is_empty());
  }
}
