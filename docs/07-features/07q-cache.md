# Query Caching

In-memory query result caching.

---

## QueryCache

```rust
pub struct QueryCache { ... }

pub struct CacheConfig {
    pub ttl_seconds: u64,
    pub max_entries: usize,
}

pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}
```

---

## Usage

```rust
let cache = QueryCache::new(CacheConfig { ttl_seconds: 300, max_entries: 1000 });
let results = cache.get("users:age:18").await?;
cache.set("users:age:18", results).await?;
let stats = cache.stats().await?;
```