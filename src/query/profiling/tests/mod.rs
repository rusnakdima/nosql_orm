use super::*;

#[test]
fn test_profiler_stats() {
  let profiler = QueryProfiler::new(10);

  profiler.record(QueryProfile::new(
    "query1".to_string(),
    "users".to_string(),
    5_000_000,
    1_000_000,
    4_000_000,
    10,
    false,
  ));

  profiler.record(QueryProfile::new(
    "query2".to_string(),
    "users".to_string(),
    15_000_000,
    2_000_000,
    13_000_000,
    5,
    true,
  ));

  assert_eq!(profiler.total_queries(), 2);
  assert_eq!(profiler.slow_query_count(), 1);
  assert!((profiler.avg_duration_ms() - 10.0).abs() < 0.1);
}
