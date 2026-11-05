# Query Caching

In-memory query result caching.

---

## QueryCache

```rust
pub struct QueryCache { ... }

pub struct QueryCacheConfig {
    pub max_size: usize,
    pub ttl_secs: Option<u64>,
    pub key_prefix: String,
}

pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub entries: usize,
}
```

---

## Usage

```rust
use nosql_orm::cache::{QueryCache, QueryCacheConfig};

let cache = QueryCache::new(QueryCacheConfig {
    max_size: 1000,
    ttl_secs: Some(300),
    ..QueryCacheConfig::default()
});
let results = cache.get::<Vec<User>>("users:age:18").await?;
cache.set("users:age:18".to_string(), &results).await?;
let stats = cache.stats().await;
println!("{} hits, {} misses", stats.hits, stats.misses);
```