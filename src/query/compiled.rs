use crate::constraints::ColumnType;
use crate::error::OrmResult;
use crate::query::Filter;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug)]
pub struct CompiledQuery {
    pub id: String,
    pub sql: String,
    pub params_schema: Vec<ColumnType>,
    pub execution_count: AtomicUsize,
    pub avg_duration_ns: AtomicU64,
    pub compiled_at: DateTime<Utc>,
}

impl Clone for CompiledQuery {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            sql: self.sql.clone(),
            params_schema: self.params_schema.clone(),
            execution_count: AtomicUsize::new(self.execution_count.load(Ordering::Relaxed)),
            avg_duration_ns: AtomicU64::new(self.avg_duration_ns.load(Ordering::Relaxed)),
            compiled_at: self.compiled_at,
        }
    }
}

impl CompiledQuery {
    pub fn new(sql: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            sql,
            params_schema: Vec::new(),
            execution_count: AtomicUsize::new(0),
            avg_duration_ns: AtomicU64::new(0),
            compiled_at: Utc::now(),
        }
    }

    pub fn record_execution(&self, duration_ns: u64) {
        let current_count = self.execution_count.load(Ordering::Relaxed);
        let current_avg = self.avg_duration_ns.load(Ordering::Relaxed);
        let new_count = current_count + 1;
        let new_avg = if current_count == 0 {
            duration_ns
        } else {
            ((current_avg * current_count as u64) + duration_ns) / new_count as u64
        };
        self.execution_count.store(new_count, Ordering::Relaxed);
        self.avg_duration_ns.store(new_avg, Ordering::Relaxed);
    }
}

#[derive(Debug)]
struct CacheEntry {
    query: CompiledQuery,
    last_accessed: AtomicU64,
    access_count: AtomicUsize,
}

impl CacheEntry {
    fn new(query: CompiledQuery) -> Self {
        Self {
            query,
            last_accessed: AtomicU64::new(Utc::now().timestamp() as u64),
            access_count: AtomicUsize::new(1),
        }
    }

    fn touch(&self) {
        self.last_accessed
            .store(Utc::now().timestamp() as u64, Ordering::Relaxed);
        self.access_count.fetch_add(1, Ordering::Relaxed);
    }
}

pub struct QueryCompiler {
    cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    max_cache_size: usize,
    hit_count: AtomicUsize,
    miss_count: AtomicUsize,
}

impl QueryCompiler {
    pub fn new(max_cache_size: usize) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            max_cache_size,
            hit_count: AtomicUsize::new(0),
            miss_count: AtomicUsize::new(0),
        }
    }

    pub async fn compile(&self, filter: &Filter, collection: &str) -> OrmResult<CompiledQuery> {
        let key = self.build_cache_key(collection, filter);

        {
            let cache = self.cache.read().await;
            if let Some(entry) = cache.get(&key) {
                entry.touch();
                self.hit_count.fetch_add(1, Ordering::Relaxed);
                return Ok(entry.query.clone());
            }
        }

        self.miss_count.fetch_add(1, Ordering::Relaxed);
        let sql = self.compile_filter_to_sql(filter);
        let query = CompiledQuery::new(sql);

        let mut guard = self.cache.write().await;
        if guard.len() >= self.max_cache_size {
            self.evict_lru(&mut guard);
        }
        guard.insert(key, CacheEntry::new(query.clone()));

        Ok(query)
    }

    fn build_cache_key(&self, collection: &str, filter: &Filter) -> String {
        format!("{}:{:?}", collection, filter)
    }

    fn compile_filter_to_sql(&self, filter: &Filter) -> String {
        match filter {
            Filter::Eq(field, value) => {
                format!("{} = {}", field, self.value_to_sql(value))
            }
            Filter::Ne(field, value) => {
                format!("{} <> {}", field, self.value_to_sql(value))
            }
            Filter::Gt(field, value) => {
                format!("{} > {}", field, self.value_to_sql(value))
            }
            Filter::Gte(field, value) => {
                format!("{} >= {}", field, self.value_to_sql(value))
            }
            Filter::Lt(field, value) => {
                format!("{} < {}", field, self.value_to_sql(value))
            }
            Filter::Lte(field, value) => {
                format!("{} <= {}", field, self.value_to_sql(value))
            }
            Filter::In(field, values) => {
                let values_str = values
                    .iter()
                    .map(|v| self.value_to_sql(v))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{} IN ({})", field, values_str)
            }
            Filter::NotIn(field, values) => {
                let values_str = values
                    .iter()
                    .map(|v| self.value_to_sql(v))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{} NOT IN ({})", field, values_str)
            }
            Filter::Contains(field, sub) => {
                format!("{} LIKE '%{}%'", field, sub)
            }
            Filter::StartsWith(field, prefix) => {
                format!("{} LIKE '{}%'", field, prefix)
            }
            Filter::EndsWith(field, suffix) => {
                format!("{} LIKE '%{}'", field, suffix)
            }
            Filter::IsNull(field) => format!("{} IS NULL", field),
            Filter::IsNotNull(field) => format!("{} IS NOT NULL", field),
            Filter::Like(field, pattern) => format!("{} LIKE '{}'", field, pattern),
            Filter::Between(field, min, max) => {
                format!(
                    "{} BETWEEN {} AND {}",
                    field,
                    self.value_to_sql(min),
                    self.value_to_sql(max)
                )
            }
            Filter::And(filters) => {
                let strs = filters
                    .iter()
                    .map(|f| format!("({})", self.compile_filter_to_sql(f)))
                    .collect::<Vec<_>>()
                    .join(" AND ");
                strs
            }
            Filter::Or(filters) => {
                let strs = filters
                    .iter()
                    .map(|f| format!("({})", self.compile_filter_to_sql(f)))
                    .collect::<Vec<_>>()
                    .join(" OR ");
                strs
            }
            Filter::Not(inner) => {
                format!("NOT ({})", self.compile_filter_to_sql(inner))
            }
        }
    }

    fn value_to_sql(&self, value: &serde_json::Value) -> String {
        match value {
            serde_json::Value::String(s) => format!("'{}'", s.replace('\'', "''")),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Null => "NULL".to_string(),
            _ => format!("'{}'", value.to_string().replace('\'', "''")),
        }
    }

    fn evict_lru(&self, cache: &mut HashMap<String, CacheEntry>) {
        let min_key = cache
            .iter()
            .min_by_key(|(_, entry)| entry.last_accessed.load(Ordering::Relaxed))
            .map(|(k, _)| k.clone());
        if let Some(key) = min_key {
            cache.remove(&key);
        }
    }

    pub async fn clear_cache(&self) {
        let mut guard = self.cache.write().await;
        guard.clear();
    }

    pub async fn cache_size(&self) -> usize {
        let guard = self.cache.read().await;
        guard.len()
    }

    pub fn cache_stats(&self) -> (usize, usize) {
        (
            self.hit_count.load(Ordering::Relaxed),
            self.miss_count.load(Ordering::Relaxed),
        )
    }
}

impl Default for QueryCompiler {
    fn default() -> Self {
        Self::new(1000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_query_compiler_caching() {
        let compiler = QueryCompiler::new(10);
        let filter = Filter::Eq("id".to_string(), serde_json::json!("123"));

        let query1 = compiler.compile(&filter, "users").await.unwrap();
        let query2 = compiler.compile(&filter, "users").await.unwrap();

        assert_eq!(query1.sql, query2.sql);
        let (hits, misses) = compiler.cache_stats();
        assert_eq!(hits, 1);
        assert_eq!(misses, 1);
    }
}