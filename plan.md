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
| PostgreSQL Provider | ✅ Implemented | tokio-postgres + deadpool-postgres | SQL |
| SQLite Provider | ✅ Implemented | rusqlite (bundled) | SQL |
| MySQL Provider | ✅ Implemented | mysql_async | SQL |

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
| **SQL Database Support** | ✅ |

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
| **Batch Relation Loading** | ✅ (RelationLoader) |

### Nice to Have
| Feature | Status |
|---------|--------|
| **Subscriptions/Pub-sub** | ✅ |
| **GraphQL Integration** | ✅ |
| **CLI Tool** | ✅ |
| **Seeding/Fixtures** | ✅ |
| **Full-text Search** | ✅ |
| **Aggregation Pipeline** | ✅ |
| **Change Data Capture (CDC)** | ✅ |

---

## 4. SQL Database Support (Implemented ✅)

### Providers Implemented

| Provider | File | Status |
|----------|------|--------|
| PostgreSQL | `src/providers/sql/postgres.rs` | ✅ |
| SQLite | `src/providers/sql/sqlite.rs` | ✅ |
| MySQL | `src/providers/sql/mysql.rs` | ✅ |

### SQL Module Structure

```
src/sql/
├── mod.rs          # Module exports
├── types.rs        # SqlDialect, SqlColumnType, SqlColumnDef, SqlIndexDef, SqlTableDef
└── query.rs        # SqlQueryBuilder for generating SQL strings

src/providers/sql/
├── mod.rs          # Provider exports (PostgresProvider, SqliteProvider, MySqlProvider)
├── postgres.rs     # PostgreSQL implementation
├── sqlite.rs       # SQLite implementation
└── mysql.rs        # MySQL implementation
```

### Key SQL Types

```rust
pub enum SqlDialect { PostgreSQL, SQLite, MySQL }

pub enum SqlColumnType {
    Serial, Integer, BigInt, Text, VarChar(u32),
    Boolean, Timestamp, DateTime, Json, JsonB, Blob
}

pub struct SqlColumnDef {
    pub name: String,
    pub column_type: SqlColumnType,
    pub nullable: bool,
    pub primary_key: bool,
    pub default: Option<String>,
    pub unique: bool,
}
```

### Usage Examples

**SQLite:**
```rust
let provider = SqliteProvider::connect("db.sqlite").await?;
let repo: Repository<User, _> = Repository::new(provider);
repo.sync_schema().await?;
```

**PostgreSQL:**
```rust
let provider = PostgresProvider::connect("postgres://user:pass@localhost/db").await?;
let repo: Repository<User, _> = Repository::new(provider);
```

**MySQL:**
```rust
let provider = MySqlProvider::connect("mysql://user:pass@localhost/db").await?;
let repo: Repository<User, _> = Repository::new(provider);
```

---

## 5. Relation Loading (Implemented ✅)

### RelationLoader

Batch loading with soft-delete filtering support:

```rust
// Load relations for multiple documents
let docs = loader.load_many(docs, &relation, filter_deleted: true).await?;

// Load relations for single document
let loaded = loader.load(&doc, &relations, filter_deleted: true).await?;
```

### RelationDef Enhancements

```rust
// Standard relations
RelationDef::one_to_many("tasks", "tasks", "todoId")
RelationDef::many_to_one("user", "users", "userId")
RelationDef::many_to_many("categories", "categories", "categories")

// Array-based relations (e.g., assignees: Vec<String>)
RelationDef::many_to_one_array("assignees", "profiles", "assignees")

// Transform loaded relation via another collection
RelationDef::many_to_one_array("assigneesProfiles", "profiles", "assignees")
    .transform_map("userId", "profiles", "id")
```

### Soft-Delete Filtering

RelationLoader automatically filters deleted records when `filter_deleted: true`:
- Filters by `deleted_at IS NULL OR deleted_at = ''`
- Post-fetches additional filtering when provider doesn't support complex filters

---

## 6. NoSQL Indexes (Implemented ✅)

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

## 7. Field Projection (SELECT/EXCLUDE) - Implemented ✅

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
```

---

## 8. Version Roadmap

| Version | Focus | Status |
|---------|-------|--------|
| 0.2.0 | Transactions + Pooling | ✅ |
| 0.3.0 | Soft Deletes + Validators + NoSQL Indexes | ✅ |
| 0.4.0 | Field Projection | ✅ |
| 0.5.0 | Migration System + CLI | ✅ |
| **0.6.0** | **SQL Providers (PostgreSQL, SQLite, MySQL)** | ✅ |
| **0.7.0** | **Batch Relation Loading (RelationLoader)** | ✅ |
| 0.8.0 | Advanced Relation Transformations | 🔲 Planned |
| 1.0.0 | Stable API + Docs | 🔲 Planned |

---

## 9. Implementation Tasks

### Completed Tasks

#### SQL Infrastructure
| Task | Priority | Status |
|------|----------|--------|
| Add SQL feature flags to Cargo.toml | High | ✅ |
| Create `src/sql/mod.rs` base trait | High | ✅ |
| Implement `SqlDialect`, `SqlColumnType`, `SqlIndexDef` types | High | ✅ |
| Create `PostgresProvider` | High | ✅ |
| Create `SqliteProvider` | High | ✅ |
| Create `MySqlProvider` | Medium | ✅ |
| Update `DatabaseProvider` trait for SQL | High | ✅ |
| Implement `SqlQueryBuilder` | High | ✅ |
| Add SQL examples | Medium | ✅ |

#### Relation Loading
| Task | Priority | Status |
|------|----------|--------|
| Create `RelationLoader` struct | High | ✅ |
| Implement `load_many` batch loading | High | ✅ |
| Add soft-delete filtering | High | ✅ |
| Add `local_key_in_array` support | High | ✅ |
| Add `transform_map_via` support | High | ✅ |

### Remaining Tasks

#### Testing
| Task | Priority | Status |
|------|----------|--------|
| Unit tests for SQL query generation | High | 🔲 |
| Integration tests for PostgreSQL | High | 🔲 |
| Integration tests for SQLite | High | 🔲 |
| Integration tests for MySQL | Medium | 🔲 |
| Hybrid query tests (SQL + NoSQL) | Medium | 🔲 |

#### Advanced Relation Loading
| Task | Priority | Status |
|------|----------|--------|
| Implement `transform_map` logic in RelationLoader | High | 🔲 |
| Complete TaskFlow integration with RelationLoader | High | 🔲 |

---

## 10. Feature Flags

```toml
[features]
default = ["json"]
json = []
mongo = ["dep:mongodb", "dep:futures-util"]
redis = ["dep:redis"]
full = ["json", "mongo", "redis"]
query_cache = []
validators = []

# SQL Providers
sql-postgres = ["dep:tokio-postgres", "dep:deadpool-postgres", "dep:base64"]
sql-sqlite = ["dep:rusqlite", "dep:base64"]
sql-mysql = ["dep:mysql_async", "dep:base64"]
sql = ["sql-postgres", "sql-sqlite", "sql-mysql"]
```

---

## 11. Module Structure

```
src/
├── lib.rs                 # Main library exports
├── entity.rs              # Entity trait
├── error.rs               # OrmError, OrmResult
├── query.rs               # Filter, QueryBuilder, SortDirection, Projection
├── relations.rs           # RelationDef, RelationLoader, WithRelations
├── repository.rs          # Repository, RelationRepository
├── soft_delete.rs         # SoftDeletable trait
├── schema.rs              # SchemaManager
├── providers/
│   ├── mod.rs             # Provider exports
│   ├── json/              # JsonProvider
│   ├── mongo/             # MongoProvider
│   ├── redis/             # RedisProvider
│   └── sql/               # PostgresProvider, SqliteProvider, MySqlProvider
├── sql/
│   ├── mod.rs             # SQL module exports
│   ├── types.rs           # SqlDialect, SqlColumnType, SqlColumnDef, etc.
│   └── query.rs           # SqlQueryBuilder
├── cache/                 # QueryCache (query_cache feature)
├── migrations/            # Migration system
├── validators/            # Entity validation
├── aggregation.rs         # Aggregation pipeline
├── cdc/                   # Change Data Capture
├── graphql/               # GraphQL integration
├── lazy/                  # Lazy loading
├── nosql_index/           # NoSQL indexes
├── pool/                  # Connection pooling
├── search/                # Full-text search
├── subscription/          # Pub/sub
└── transaction.rs         # Transaction support
```

---

## 12. Examples

```rust
// JSON (default)
let provider = JsonProvider::new("./data").await?;

// MongoDB
let provider = MongoProvider::connect("mongodb://localhost:27017").await?;

// PostgreSQL
let provider = PostgresProvider::connect("postgres://user:pass@localhost/db").await?;

// SQLite
let provider = SqliteProvider::connect("app.db").await?;

// MySQL
let provider = MySqlProvider::connect("mysql://user:pass@localhost/db").await?;

let repo: Repository<Entity, _> = Repository::new(provider);
```