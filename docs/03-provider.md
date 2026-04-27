# Database Providers

Providers abstract database operations, allowing the same ORM logic to work with different backends.

---

## Table of Contents

1. [Provider Interface](#provider-interface)
2. [JSON Provider](#json-provider)
3. [MongoDB Provider](#mongodb-provider)
4. [Redis Provider](#redis-provider)
5. [SQL Providers](#sql-providers)

---

## Provider Interface

### DatabaseProvider Trait

```rust
#[async_trait]
pub trait DatabaseProvider: Send + Sync + Clone + 'static {
    async fn insert(&self, collection: &str, doc: Value) -> OrmResult<Value>;
    async fn find_by_id(&self, collection: &str, id: &str) -> OrmResult<Option<Value>>;
    async fn find_many(&self, collection: &str, filter: Option<&Filter>, skip: Option<u64>, limit: Option<u64>, sort_by: Option<&str>, sort_asc: bool) -> OrmResult<Vec<Value>>;
    async fn update(&self, collection: &str, id: &str, doc: Value) -> OrmResult<Value>;
    async fn patch(&self, collection: &str, id: &str, patch: Value) -> OrmResult<Value>;
    async fn delete(&self, collection: &str, id: &str) -> OrmResult<bool>;
    async fn delete_many(&self, collection: &str, filter: Option<Filter>) -> OrmResult<usize>;
    async fn update_many(&self, collection: &str, filter: Option<Filter>, updates: Value) -> OrmResult<usize>;
    async fn count(&self, collection: &str, filter: Option<&Filter>) -> OrmResult<u64>;
    async fn exists(&self, collection: &str, id: &str) -> OrmResult<bool>;
    async fn find_all(&self, collection: &str) -> OrmResult<Vec<Value>> {
        self.find_many(collection, None, None, None, None, true).await
    }
    async fn create_index(&self, collection: &str, index: &NosqlIndex) -> OrmResult<()>;
    async fn drop_index(&self, collection: &str, index_name: &str) -> OrmResult<()>;
    async fn list_indexes(&self, collection: &str) -> OrmResult<Vec<NosqlIndexInfo>>;
    async fn index_exists(&self, collection: &str, index_name: &str) -> OrmResult<bool> {
        let indexes = self.list_indexes(collection).await?;
        Ok(indexes.iter().any(|i| i.name == index_name))
    }
}
```

### ProviderConfig

```rust
pub struct ProviderConfig {
    pub connection: String,
    pub database: Option<String>,
    pub options: HashMap<String, String>,
}

impl ProviderConfig {
    pub fn new(connection: impl Into<String>) -> Self;
    pub fn with_database(mut self, db: impl Into<String>) -> Self;
    pub fn with_option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self;
}
```

---

## JSON Provider

File-based JSON storage. Each collection is stored as a JSON array in `<base_dir>/<collection>.json`.

### Creating JsonProvider

```rust
use nosql_orm::prelude::*;

// Create/open JSON database at directory
let provider = JsonProvider::new("./data").await?;
let provider = JsonProvider::new("/path/to/data").await?;

// With temp directory (for testing)
let temp_dir = tempfile::tempdir()?;
let provider = JsonProvider::new(temp_dir.path()).await?;
```

### Operations

All operations go through an in-memory cache (RwLock), then flushed to disk:

```
insert() ──► In-memory cache ──► Flush (pretty printed JSON)
find_by_id() ──► In-memory cache ──► Load from disk if not cached
update() ──► In-memory cache ──► Mark dirty ──► Flush
patch() ──► Merge fields ──► Flush
delete() ──► Remove from cache ──► Flush
```

### Example

```rust
use nosql_orm::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Option<String>,
    pub name: String,
    pub email: String,
}

impl Entity for User {
    fn meta() -> EntityMeta { EntityMeta::new("users") }
    fn get_id(&self) -> Option<String> { self.id.clone() }
    fn set_id(&mut self, id: String) { self.id = Some(id); }
}

#[tokio::main]
async fn main() -> OrmResult<()> {
    let provider = JsonProvider::new("./data").await?;
    let repo: Repository<User, _> = Repository::new(provider);

    // Create
    let user = repo.save(User {
        id: None,
        name: "Alice".into(),
        email: "alice@example.com".into(),
    }).await?;

    // Read
    let found = repo.find_by_id(&user.id.unwrap()).await?;

    // Update
    let updated = repo.save(User {
        id: user.id.clone(),
        name: "Alice Updated".into(),
        ..user
    }).await?;

    // Delete
    repo.delete(&user.id.unwrap()).await?;

    Ok(())
}
```

---

## MongoDB Provider

MongoDB driver integration.

### Creating MongoProvider

```rust
use nosql_orm::prelude::*;

// Direct connection
let provider = MongoProvider::connect("mongodb://localhost:27017", "mydb").await?;

// Via config
let config = ProviderConfig::new("mongodb://localhost:27017")
    .with_database("mydb");
let provider = MongoProvider::from_config(&config).await?;

// With options
let config = ProviderConfig::new("mongodb://localhost:27017")
    .with_database("mydb")
    .with_option("maxPoolSize", "10");
let provider = MongoProvider::from_config(&config).await?;
```

### ID Mapping

MongoDB uses `_id`, the library maps to/from `id`:

```rust
// When storing: id → _id
// When reading: _id → id
```

### Filter Mapping

Filters are converted to MongoDB query documents:

```rust
Filter::Eq("name", "Alice")         // → { name: "Alice" }
Filter::Contains("name", "lic")      // → { name: { $regex: "lic", $options: "i" } }
Filter::StartsWith("name", "A")    // → { name: { $regex: "^A", $options: "i" } }
Filter::Gt("age", 18)             // → { age: { $gt: 18 } }
Filter::In("status", ["a", "b"])   // → { status: { $in: ["a", "b"] } }
Filter::And(filters)            // → { $and: [...] }
Filter::Or(filters)             // → { $or: [...] }
```

### Example

```rust
use nosql_orm::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Option<String>,
    pub name: String,
    pub email: String,
}

impl Entity for User {
    fn meta() -> EntityMeta { EntityMeta::new("users") }
    fn get_id(&self) -> Option<String> { self.id.clone() }
    fn set_id(&mut self, id: String) { self.id = Some(id); }
}

#[tokio::main]
async fn main() -> OrmResult<()> {
    let provider = MongoProvider::connect("mongodb://localhost:27017", "mydb").await?;
    let repo: Repository<User, _> = Repository::new(provider);

    let user = repo.save(User {
        id: None,
        name: "Alice".into(),
        email: "alice@example.com".into(),
    }).await?;

    println!("Saved user: {:?}", user);
    Ok(())
}
```

---

## Redis Provider

Redis key-value storage, caching, pub/sub.

### Creating RedisProvider

```rust
use nosql_orm::prelude::*;

// Direct connection
let provider = RedisProvider::connect("redis://localhost:6379").await?;

// With password
let provider = RedisProvider::connect("redis://user:password@localhost:6379").await?;

// With database number
let provider = RedisProvider::connect("redis://localhost:6379/1").await?;
```

### Operations

Redis provider stores JSON values:

```rust
// Insert: collection:id → JSON
// Find by id: collection:id → JSON
// Find many: collection:* → JSON
// Delete: collection:id → DEL
```

### Example

```rust
#[tokio::main]
async fn main() -> OrmResult<()> {
    let provider = RedisProvider::connect("redis://localhost:6379").await?;
    let repo: Repository<User, _> = Repository::new(provider);

    // Use for caching
    let user = repo.save(User {
        id: Some("user:1".into()),
        name: "Alice".into(),
        email: "alice@example.com".into(),
    }).await?;

    Ok(())
}
```

---

## SQL Providers

### PostgreSQL Provider

```rust
#[cfg(feature = "sql-postgres")]
let provider = PostgresProvider::connect(
    "host=localhost user=postgres password=secret dbname=mydb"
).await?;

// Or using URL
let provider = PostgresProvider::connect(
    "postgres://postgres:secret@localhost/mydb"
).await?;
```

### SQLite Provider

```rust
#[cfg(feature = "sql-sqlite")]
let provider = SqliteProvider::connect("app.db").await?;

// In-memory
let provider = SqliteProvider::connect(":memory:").await?;
```

### MySQL Provider

```rust
#[cfg(feature = "sql-mysql")]
let provider = MySqlProvider::connect(
    "mysql://user:password@localhost:3306/mydb"
).await?;
```

---

## Swapping Providers

The same code works with any provider:

```rust
async fn create_user<P: DatabaseProvider>(provider: P) -> OrmResult<()> {
    let repo: Repository<User, P> = Repository::new(provider);
    let user = repo.save(User {
        id: None,
        name: "Alice".into(),
        email: "alice@example.com".into(),
    }).await?;
    Ok(())
}

// Works with JSON
create_user(JsonProvider::new("./data").await?).await?;

// Works with MongoDB
create_user(MongoProvider::connect("mongodb://localhost:27017", "mydb").await?).await?;

// Works with PostgreSQL
#[cfg(feature = "sql-postgres")]
create_user(PostgresProvider::connect("postgres://user:pass@localhost/db").await?).await?;
```

---

## Provider Comparison

| Feature | JSON | MongoDB | Redis | SQL |
|--------|------|--------|------|------|
| Storage | Files | MongoDB | Redis | SQL DB |
| Transactions | ❌ | ✅ | ✅ | ✅ |
| Indexes | ⚠️ No-op | ✅ native | ✅ | ✅ |
| Query language | In-memory | MongoQL | Key-scan | SQL |
| Relations | ✅ | ✅ | ⚠️ Basic | ✅ |
| Scalability | Single instance | Replica set | Cluster | Single/replica |
| Zero-config | ✅ | ❌ | ❌ | ❌ |

---

## Next Steps

- [04-repository.md](04-repository.md) - Repository CRUD operations
- [05-query-builder.md](05-query-builder.md) - Query building
- [06-relations.md](06-relations.md) - Relations
- [07-features/07f-indexes.md](07-features/07f-indexes.md) - Indexes