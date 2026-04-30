# Query Logging

Log queries for debugging and monitoring.

---

## Architecture

The logging system uses a **wrapper pattern** for clean separation:

```rust
// Logging strategy trait - implemented by loggers
pub trait LoggingStrategy {
    fn log_start(&self, operation: &str, collection: &str);
    fn log_complete(&self, operation: &str, collection: &str, duration_ms: u64, success: bool);
    fn log_error(&self, operation: &str, collection: &str, error: &str);
}

// Generic wrapper that wraps any DatabaseProvider with any LoggingStrategy
pub struct ProviderWrapper<P, L> {
    inner: P,
    logger: L,
}
```

---

## LoggingStrategy Trait

```rust
pub trait LoggingStrategy: Send + Sync {
    fn log_start(&self, operation: &str, collection: &str);
    fn log_complete(&self, operation: &str, collection: &str, duration_ms: u64, success: bool);
    fn log_error(&self, operation: &str, collection: &str, error: &str);
}
```

### Implementations

| Logger | Description |
|--------|-------------|
| `FileQueryLogger` | Logs to file |
| `DbQueryLogger` | Logs to database collection |
| `QueryLogger` | Console logging |

---

## ProviderWrapper

Wraps any `DatabaseProvider` with logging:

```rust
// Create base provider
let json_provider = JsonProvider::new("./data").await?;

// Create logger
let file_logger = FileQueryLogger::new("./queries.log").await?;

// Wrap provider with logging
let wrapped = ProviderWrapper::new(json_provider, file_logger);

// Use wrapped provider - all operations are logged
let repo = Repository::<User, _>::new(wrapped);
```

### Chaining

Logging can be stacked:

```rust
let wrapped = ProviderWrapper::new(json_provider, file_logger);
let with_db_logging = ProviderWrapper::new(wrapped, db_logger);
```

---

## DbQueryLogger

Logs queries to a database collection:

```rust
use nosql_orm::logging::{DbQueryLogger, ProviderWrapper};

// Create provider
let json_provider = JsonProvider::new("./data").await?;

// Create DB logger (logs to "query_logs" collection)
let db_logger = DbQueryLogger::new(json_provider);

// Wrap with logging
let wrapped = ProviderWrapper::new(json_provider, db_logger);

// All operations are logged to database
let repo = Repository::<User, _>::new(wrapped);
```

### Log Entry Structure

```rust
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub level: String,       // "INFO", "DEBUG", "WARN", "ERROR"
    pub operation: String,   // "insert", "find_by_id", "update", etc.
    pub collection: String,   // Collection name
    pub duration_ms: u64,     // Query duration
    pub success: bool,
    pub error: Option<String>,
}
```

---

## FileQueryLogger

Logs to file with rotation:

```rust
use nosql_orm::logging::{FileQueryLogger, ProviderWrapper};

let file_logger = FileQueryLogger::new("./logs/queries.log").await?;
let wrapped = ProviderWrapper::new(provider, file_logger);
```

---

## QueryLogger (Console)

Simple console logging:

```rust
use nosql_orm::logging::{QueryLogger, ProviderWrapper};

let console_logger = QueryLogger::new();
let wrapped = ProviderWrapper::new(provider, console_logger);
```

---

## Usage Example

```rust
use nosql_orm::prelude::*;
use nosql_orm::logging::{FileQueryLogger, ProviderWrapper};

#[derive(Debug, Clone, Serialize, Deserialize, Model)]
#[table_name("users")]
pub struct User {
    pub id: Option<String>,
    pub name: String,
    pub email: String,
}

#[tokio::main]
async fn main() -> OrmResult<()> {
    // Setup with logging
    let provider = JsonProvider::new("./data").await?;
    let logger = FileQueryLogger::new("./queries.log").await?;
    let wrapped = ProviderWrapper::new(provider, logger);

    let repo = Repository::<User, _>::new(wrapped);

    // All operations are logged
    repo.save(User {
        id: None,
        name: "Alice".into(),
        email: "alice@example.com".into(),
    }).await?;

    let users = repo.find_all().await?;

    Ok(())
}
```

---

## Configuration

### Max Log Entries

```rust
let logger = DbQueryLogger::builder()
    .max_logs(10000)          // Maximum log entries
    .retention_count(1000)   // Entries to keep after cleanup
    .build(provider);
```

### Log Levels

Loggers support different levels:

```rust
// INFO - all operations
// DEBUG - detailed timing
// WARN - slow queries
// ERROR - failed operations
```

---

## Next Steps

- [07-features/07q-cache.md](07q-cache.md) - Query caching
- [07-features/07e-migrations.md](07e-migrations.md) - Migration system