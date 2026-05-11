use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

#[derive(Debug, Clone)]
pub struct QueryProfile {
    pub query_id: String,
    pub collection: String,
    pub duration_ns: u64,
    pub planning_ns: u64,
    pub execution_ns: u64,
    pub result_count: usize,
    pub cache_hit: bool,
}

impl QueryProfile {
    pub fn new(
        query_id: String,
        collection: String,
        duration_ns: u64,
        planning_ns: u64,
        execution_ns: u64,
        result_count: usize,
        cache_hit: bool,
    ) -> Self {
        Self {
            query_id,
            collection,
            duration_ns,
            planning_ns,
            execution_ns,
            result_count,
            cache_hit,
        }
    }

    pub fn duration_ms(&self) -> f64 {
        self.duration_ns as f64 / 1_000_000.0
    }

    pub fn planning_ms(&self) -> f64 {
        self.planning_ns as f64 / 1_000_000.0
    }

    pub fn execution_ms(&self) -> f64 {
        self.execution_ns as f64 / 1_000_000.0
    }
}

pub struct QueryProfiler {
    total_queries: AtomicUsize,
    total_duration_ns: AtomicU64,
    slow_threshold_ns: u64,
    cache_hits: AtomicUsize,
    cache_misses: AtomicUsize,
    slow_queries: AtomicUsize,
    total_results: AtomicUsize,
}

impl QueryProfiler {
    pub fn new(slow_threshold_ms: u64) -> Self {
        Self {
            total_queries: AtomicUsize::new(0),
            total_duration_ns: AtomicU64::new(0),
            slow_threshold_ns: slow_threshold_ms * 1_000_000,
            cache_hits: AtomicUsize::new(0),
            cache_misses: AtomicUsize::new(0),
            slow_queries: AtomicUsize::new(0),
            total_results: AtomicUsize::new(0),
        }
    }

    pub fn record(&self, profile: QueryProfile) {
        self.total_queries.fetch_add(1, Ordering::Relaxed);
        self.total_duration_ns
            .fetch_add(profile.duration_ns, Ordering::Relaxed);
        self.total_results
            .fetch_add(profile.result_count, Ordering::Relaxed);

        if profile.cache_hit {
            self.cache_hits.fetch_add(1, Ordering::Relaxed);
        } else {
            self.cache_misses.fetch_add(1, Ordering::Relaxed);
        }

        if profile.duration_ns > self.slow_threshold_ns {
            self.slow_queries.fetch_add(1, Ordering::Relaxed);
            #[cfg(feature = "logging")]
            {
                log::warn!(
                    "Slow query detected: collection={} duration_ms={:.3} results={}",
                    profile.collection,
                    profile.duration_ms(),
                    profile.result_count
                );
            }
        }
    }

    pub fn avg_duration_ms(&self) -> f64 {
        let total = self.total_duration_ns.load(Ordering::Relaxed);
        let count = self.total_queries.load(Ordering::Relaxed);
        if count == 0 {
            0.0
        } else {
            (total / count as u64) as f64 / 1_000_000.0
        }
    }

    pub fn cache_hit_rate(&self) -> f64 {
        let hits = self.cache_hits.load(Ordering::Relaxed) as f64;
        let misses = self.cache_misses.load(Ordering::Relaxed) as f64;
        let total = hits + misses;
        if total == 0.0 {
            0.0
        } else {
            hits / total
        }
    }

    pub fn slow_query_count(&self) -> usize {
        self.slow_queries.load(Ordering::Relaxed)
    }

    pub fn total_queries(&self) -> usize {
        self.total_queries.load(Ordering::Relaxed)
    }

    pub fn total_results(&self) -> usize {
        self.total_results.load(Ordering::Relaxed)
    }

    pub fn reset(&self) {
        self.total_queries.store(0, Ordering::Relaxed);
        self.total_duration_ns.store(0, Ordering::Relaxed);
        self.cache_hits.store(0, Ordering::Relaxed);
        self.cache_misses.store(0, Ordering::Relaxed);
        self.slow_queries.store(0, Ordering::Relaxed);
        self.total_results.store(0, Ordering::Relaxed);
    }
}

impl Default for QueryProfiler {
    fn default() -> Self {
        Self::new(100)
    }
}

pub struct ScopedTimer {
    start_ns: u64,
}

impl ScopedTimer {
pub fn new() -> Self {
            Self {
                start_ns: std::time::Instant::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .as_nanos() as u64,
            }
        }

    pub fn elapsed_ns(&self) -> u64 {
        std::time::Instant::now()
            .duration_since(std::time::UNIX_EPOCH)
            .as_nanos() as u64
            .saturating_sub(self.start_ns)
    }

    pub fn elapsed_ms(&self) -> f64 {
        self.elapsed_ns() as f64 / 1_000_000.0
    }
}

impl Default for ScopedTimer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
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
}