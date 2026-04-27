# Query Logging

Log queries for debugging.

---

## QueryLogger

```rust
pub trait QueryLogger {
    async fn log(&self, query: &str, duration: Duration);
}
```

## Implementations

```rust
// File logger
pub struct FileQueryLogger { path: PathBuf }

// Database logger
pub struct DbQueryLogger { collection: String }
```

---

## Usage

```rust
let logger = FileQueryLogger::new("./queries.log").await?;
repo.set_logger(logger);
```