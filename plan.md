# nosql_orm - Implementation Plan

## 1. Library Verification

**Status: ✅ Complete**

Library properly initialized as Rust library with `src/lib.rs`, prelude exports, and feature flags.

---

## 2. Current Database Integrations

| Provider | Status | Backend | Type |
|----------|--------|---------|------|
| JSON Provider | ✅ Implemented | File-based JSON storage (embedded, zero-config) | NoSQL/Document |
| MongoDB Provider | ✅ Implemented | MongoDB driver v2 | NoSQL/Document |
| Redis Provider | ✅ Implemented | Caching, pub/sub, sessions, streams | NoSQL/Key-Value |
| **PostgreSQL Provider** | 🔲 Planned | SQL relational | SQL |
| **SQLite Provider** | 🔲 Planned | SQL relational | SQL |
| **MySQL Provider** | 🔲 Planned | SQL relational | SQL |

---

## 3. Implemented Features

### Critical
| Feature | Status |
|---------|--------|
| **Migration System** | ✅ |
| **Connection Pooling** | ✅ |
| **Transaction Support** | ✅ |
| **Soft Deletes** | ✅ |
| **Query Caching** | ✅ |
| **Batch Operations** | ✅ |
| **Field Projection (select/exclude)** | ✅ |

### Important
| Feature | Status |
|---------|--------|
| **Lazy Loading** | ✅ |
| **Event Listeners** | ✅ |
| **Entity Validation** | ✅ |
| **Automatic ID Generation** | ✅ |
| **Multi-tenancy** | ✅ |
| **Embedded Entities** | ✅ |
| **Inheritance** | ✅ |
| **NoSQL Indexes** | ✅ |

### Nice to Have
| Feature | Status |
|---------|--------|
| **Subscriptions/Pub-sub** | ✅ |
| **GraphQL Integration** | ✅ |
| **CLI Tool** | ✅ |
| **Seeding/Fixtures** | ✅ |
| **Full-text Search** | ✅ |
| **Aggregation Pipeline** | ✅ |
| **Change Data Capture** | ✅ |

---

## 4. SQL Database Support (New)

### Motivation

The user wants to use nosql_orm in **hybrid mode** - using SQL databases alongside NoSQL databases in the same application. This enables:

- **Symbiotic architecture**: SQL for transactional data, NoSQL for flexible documents
- **Gradual migration**: Start with SQL, migrate to NoSQL for specific entities
- **Best-of-both-worlds**: Use the right database for each use case
- **Unified API**: Same Entity/Repository patterns across both database types

### SQL Providers to Implement

| Provider | Priority | Use Case |
|----------|----------|----------|
| **PostgreSQL** | High | Primary SQL backend, advanced features (JSONB, full-text) |
| **SQLite** | High | Embedded/local apps, testing |
| **MySQL** | Medium | Legacy systems, shared hosting |

### Architecture

```
┌─────────────────────────────────────────────────────────┐
│                      Application                        │
├─────────────────────────────────────────────────────────┤
│  Entity<T> + Repository<T, P> + QueryBuilder           │
├─────────────┬─────────────────┬────────────────────────┤
│  JsonRepo   │   MongoRepo     │   SqlRepo              │
├─────────────┼─────────────────┼────────────────────────┤
│  JSON       │   MongoDB       │   PostgreSQL           │
│  Provider   │   Provider      │   Provider             │
└─────────────┴─────────────────┴────────────────────────┘
```

### Key Design Decisions

1. **Unified Entity Trait**: Same `Entity` trait works for all providers
2. **Provider-Specific Configuration**: Each provider has its own config options
3. **Schema Mapping**: SQL providers map entity to tables, NoSQL maps to collections
4. **Transaction Unification**: Both SQL and NoSQL support ACID transactions
5. **Index Abstraction**: SQL uses traditional indexes, NoSQL uses MongoDB-style indexes

---

## 5. SQL Implementation Plan

### Phase 1: Core Infrastructure

**5.1 SQL Provider Trait**

Create `src/providers/sql/mod.rs` with base SQL provider interface:

```rust
/// SQL dialect enumeration
pub enum SqlDialect {
    PostgreSQL,
    SQLite,
    MySQL,
}

/// SQL column types
pub enum SqlColumnType {
    Integer,
    BigInt,
    Text,
    VarChar(u32),
    Boolean,
    Timestamp,
    DateTime,
    Json,
    JsonB,
    Blob,
}

/// SQL index definition
pub struct SqlIndexDef {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
    pub index_type: SqlIndexType,
}

pub enum SqlIndexType {
    BTree,     // Default, good for equality/range
    Hash,      // Fast equality lookups
    GiST,      // Geometric/experimental
    GIN,       // Full-text search, JSON
}

/// SQL column definition
pub struct SqlColumnDef {
    pub name: String,
    pub column_type: SqlColumnType,
    pub nullable: bool,
    pub primary_key: bool,
    pub default: Option<String>,
    pub unique: bool,
}
```

**5.2 Add SQL features to Cargo.toml**

```toml
[features]
# ... existing ...
sql = ["dep:sqlx", "dep:tokio-postgres"]
sql-sqlite = ["dep:rusqlite"]
sql-mysql = ["dep:mysql"]
```

### Phase 2: PostgreSQL Provider

**New file: `src/providers/sql/postgres.rs`**

Features:
- Full SQL-92 support via `tokio-postgres` or `sqlx`
- JSON/JSONB columns for hybrid storage
- Full-text search indexes (GIN/GiST)
- Connection pooling via deadpool
- Transaction support with SAVEPOINTs

Key methods to implement:
```rust
pub struct PostgresProvider {
    pool: Pool<PostgresConnectionManager>,
}

impl PostgresProvider {
    pub async fn connect(connection_string: &str) -> OrmResult<Self>;
    pub async fn execute_sql(&self, sql: &str) -> OrmResult<u64>;
    pub async fn query_sql(&self, sql: &str) -> OrmResult<Vec<Row>>;
    pub async fn create_table(&self, name: &str, columns: &[SqlColumnDef]) -> OrmResult<()>;
    pub async fn drop_table(&self, name: &str) -> OrmResult<()>;
    pub async fn create_index(&self, index: &SqlIndexDef) -> OrmResult<()>;
    pub async fn alter_table_add_column(&self, table: &str, column: &SqlColumnDef) -> OrmResult<()>;
}
```

### Phase 3: SQLite Provider

**New file: `src/providers/sql/sqlite.rs`**

Features:
- Embedded database (no server needed)
- Perfect for testing and local development
- WAL mode for concurrent reads
- Full-text search via FTS5 extension

### Phase 4: MySQL Provider

**New file: `src/providers/sql/mysql.rs`**

Features:
- `mysql_async` driver for async operations
- Connection pooling
- Limited JSON support (later versions have JSON type)

### Phase 5: Unified Query Builder

**Update `src/query.rs` to support SQL**

```rust
/// Query builder that works for both SQL and NoSQL
pub struct QueryBuilder<P: DatabaseProvider> {
    provider: P,
    filter: Option<Filter>,
    order_by: Vec<(String, SortDirection)>,
    limit: Option<u32>,
    offset: Option<u32>,
    // SQL-specific
    selected_fields: Option<Vec<String>>,
    group_by: Option<Vec<String>>,
    having: Option<Filter>,
}

/// Build SQL query string
impl QueryBuilder<SqlProvider> {
    pub fn build_select(&self, table: &str) -> String {
        // Generate: SELECT ... FROM ... WHERE ... ORDER BY ... LIMIT ...
    }

    pub fn build_insert(&self, table: &str, data: &Value) -> String;
    pub fn build_update(&self, table: &str, data: &Value, filter: &Filter) -> String;
    pub fn build_delete(&self, table: &str, filter: &Filter) -> String;
}
```

### Phase 6: Schema Manager Integration

**Update `src/schema/schema.rs`**

```rust
/// Unified schema manager for both SQL and NoSQL
pub struct SchemaManager<P: DatabaseProvider> {
    provider: P,
}

impl<P: DatabaseProvider> SchemaManager<P> {
    pub async fn create_collection(&self, name: &str) -> OrmResult<()>;
    pub async fn drop_collection(&self, name: &str) -> OrmResult<()>;
    pub async fn sync_entity<T: Entity>(&self) -> OrmResult<()>;
}
```

---

## 6. NoSQL Indexes (Implemented)

### MongoDB Index Types

| Index Type | Description |
|------------|-------------|
| **Single Field** | Basic queries on one field |
| **Compound** | Multi-field queries |
| **Text** | Full-text search |
| **Geospatial** | Location queries (2dsphere, 2d) |
| **TTL** | Auto-expiration |
| **Hashed** | Hash-based for sharding |

### Key Methods

```rust
// Create single field index
repo.create_index(NosqlIndex::single("email", 1).unique(true)).await?;

// Create compound index
repo.create_index(NosqlIndex::compound(&[("user_id", 1), ("date", -1)])).await?;

// Create TTL index (auto-delete)
repo.create_index(NosqlIndex::ttl("created_at", 30 * 24 * 60 * 60)).await?;

// Using IndexManager
repo.indexes().create_text_index(&[("title", 10), ("body", 5)], Some("en")).await?;

// Sync from entity definition
repo.sync_indexes().await?;
```

---

## 7. Field Projection (SELECT/EXCLUDE)

### Implemented ✅

```rust
// Select only specific fields
repo.query()
    .select(&["id", "name", "email"])
    .find()
    .await?;

// Exclude specific fields (e.g., passwords)
repo.query()
    .exclude(&["password", "token"])
    .find()
    .await?;

// Combine with filters
repo.query()
    .where_gt("age", serde_json::json!(18))
    .select(&["id", "name", "age"])
    .find()
    .await?;

// Get raw JSON with projection
repo.query()
    .exclude(&["password"])
    .find_raw()
    .await?;
```

---

## 8. Version Roadmap

| Version | Focus | Status |
|---------|-------|--------|
| 0.2.0 | Transactions + Pooling | ✅ |
| 0.3.0 | Soft Deletes + Validators | ✅ |
| 0.4.0 | Field Projection | ✅ |
| 0.5.0 | Migration System + CLI | ✅ |
| **0.6.0** | **SQL Providers (PostgreSQL, SQLite, MySQL)** | **🔲 Planned** |
| 0.7.0 | SQL-NoSQL Hybrid Queries | 🔲 Planned |
| 0.8.0 | Elasticsearch Provider | 🔲 Planned |
| 1.0.0 | Stable API + Docs | 🔲 Planned |

---

## 9. Implementation Tasks

### SQL Infrastructure

| Task | Priority | Status |
|------|----------|--------|
| Add SQL feature flags to Cargo.toml | High | 🔲 |
| Create `src/providers/sql/mod.rs` base trait | High | 🔲 |
| Implement `SqlDialect`, `SqlColumnType`, `SqlIndexDef` types | High | 🔲 |
| Create `PostgresProvider` | High | 🔲 |
| Create `SqliteProvider` | High | 🔲 |
| Create `MySqlProvider` | Medium | 🔲 |
| Update `DatabaseProvider` trait for SQL | High | 🔲 |
| Implement `QueryBuilder.build_sql()` | Medium | 🔲 |
| Update `Repository` for SQL providers | Medium | 🔲 |
| Update `SchemaManager` for SQL | Medium | 🔲 |
| Add SQL examples | Medium | 🔲 |

### Testing

| Task | Priority | Status |
|------|----------|--------|
| Unit tests for SQL query generation | High | 🔲 |
| Integration tests for PostgreSQL | High | 🔲 |
| Integration tests for SQLite | High | 🔲 |
| Hybrid query tests (SQL + NoSQL) | Medium | 🔲 |

---

## 10. Usage Examples (Future)

### PostgreSQL

```rust
use nosql_orm::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize, Entity)]
pub struct User {
    pub id: Option<i32>,
    pub name: String,
    pub email: String,
}

impl Entity for User {
    fn meta() -> EntityMeta {
        EntityMeta::new("users").sql_table("users")
    }
    fn get_id(&self) -> Option<i32> { self.id }
    fn set_id(&mut self, id: i32) { self.id = Some(id); }
}

#[tokio::main]
async fn main() -> OrmResult<()> {
    let provider = PostgresProvider::connect("postgres://user:pass@localhost/db").await?;
    let repo: Repository<User, _> = Repository::new(provider);

    // Create table from entity
    repo.sync_schema().await?;

    // CRUD operations
    let user = User { id: None, name: "Alice".into(), email: "alice@example.com".into() };
    let saved = repo.save(user).await?;

    // Query with filters
    let users = repo.query()
        .where_eq("email", serde_json::json!("alice@example.com"))
        .find()
        .await?;

    Ok(())
}
```

### Hybrid SQL + NoSQL

```rust
use nosql_orm::prelude::*;

// SQL provider for transactional data
let pg = PostgresProvider::connect("postgres://...").await?;
// NoSQL provider for flexible documents
let mongo = MongoProvider::connect("mongodb://...").await?;

let user_repo: Repository<User, _> = Repository::new(pg);
let document_repo: Repository<Document, _> = Repository::new(mongo);

// User data in PostgreSQL (structured, ACID)
let user = user_repo.save(User { id: None, name: "Bob".into() }).await?;

// Document data in MongoDB (flexible schema)
let doc = document_repo.save(Document { id: None, content: json!({...}) }).await?;
```