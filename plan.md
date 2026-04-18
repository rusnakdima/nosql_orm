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

---

## 5. Relation Loading (Implemented ✅)

### RelationLoader

Batch loading with soft-delete filtering support.

---

## 6. NoSQL Indexes (Implemented ✅)

---

## 7. Planned Features (from Popular ORMs)

Based on analysis of TypeORM, Django ORM, Prisma, SQLAlchemy, and Entity Framework.

### High Priority Features

#### 7.1 Query Builder Enhancements

| Feature | Description | Status |
|---------|-------------|--------|
| **Chainable Query Methods** | Fluent API: `.where().orderBy().limit()` | ✅ Implemented |
| **Complex OR/AND Filters** | Nested filter groups with `Filter::Or`, `Filter::And` | ✅ Implemented |
| **Cursor-based Pagination** | More efficient than offset for large datasets | ✅ Implemented |
| **Query Result Streaming** | Stream results instead of loading all into memory | 🔲 Planned |
| **Raw Query Execution** | `repo.query().raw("SELECT * FROM users WHERE age > ?", &[18])` | 🔲 Planned |

**Example:**
```rust
// Chainable query
let users = repo.query()
    .where("age").gte(18)
    .and("status").eq("active")
    .order_by("created_at", SortDirection::Desc)
    .limit(20)
    .offset(40)
    .find()
    .await?;

// Cursor-based pagination
let (users, next_cursor) = repo.query()
    .where("age").gt(last_age)
    .limit(20)
    .find_with_cursor()
    .await?;
```

#### 7.2 Bulk Data Operations

| Feature | Description | Status |
|---------|-------------|--------|
| **Bulk Insert** | Insert multiple records in single query | ✅ Implemented |
| **Bulk Update** | Update multiple records matching filter | ✅ Implemented |
| **Bulk Upsert** | Insert or update based on unique constraint | ✅ Implemented |
| **Batch Delete** | Delete multiple records efficiently | ✅ Implemented |

**Example:**
```rust
// Bulk insert
repo.insert_many(users_vec).await?;

// Bulk upsert
repo.upsert_many(users_vec, ["email"]).await?;

// Batch update
repo.update_many("users", filter, updates).await?;
```

#### 7.3 Change Tracking & Dirty Checking

| Feature | Description | Status |
|---------|-------------|--------|
| **Entity Change Detection** | Track modified fields before save | 🔲 Planned |
| **Optimistic Locking** | Version field for concurrent update detection | 🔲 Planned |
| **Auto-timestamp Updates** | Update `updated_at` automatically on change | ✅ Implemented |

**Example:**
```rust
// Dirty checking
entity.name = "New Name";
entity.age = 30;
let changes = entity.get_changes(); // { name: "Old", age: 25 }
repo.save(entity).await?; // Only updates changed fields

// Optimistic locking
#[entity]
pub struct User {
    #[column(version)]
    pub version: i32,
}
// UPDATE users SET ... WHERE id = ? AND version = ?
```

#### 7.4 Advanced Filters

| Feature | Description | Status |
|---------|-------------|--------|
| **Like/Contains/StartsWith/EndsWith** | String pattern matching | ✅ Implemented |
| **Between** | Range queries for numbers/dates | ✅ Implemented |
| **In/NOT In** | Array membership checks | ✅ Implemented |
| **IsNull/IsNotNull** | Null checks | ✅ Implemented |
| **Json Path Queries** | Query JSON fields deeply | 🔲 Planned |

**Example:**
```rust
repo.query()
    .where("name").like("%John%")
    .where("age").between(18, 65)
    .where("status").in_(["active", "pending"])
    .where("meta").json_path("$.role").eq("admin")
    .find()
    .await?;
```

### Medium Priority Features

#### 7.5 Transaction Management

| Feature | Description | Status |
|---------|-------------|--------|
| **Savepoints** | Nested transaction support | 🔲 Planned |
| **Transaction Callbacks** | `with_transaction(\|tx\| async { ... })` | 🔲 Planned |
| **Isolation Levels** | READ COMMITTED, SERIALIZABLE, etc. | 🔲 Planned |
| **Retry on Deadlock** | Automatic retry logic for failed transactions | 🔲 Planned |

**Example:**
```rust
// Transaction with callback
repo.with_transaction(|tx| async {
    tx.save(user).await?;
    tx.insert(Order { user_id: user.id, .. }).await?;
    Ok(())
}).await?;

// Isolation level
repo.with_isolation(IsolationLevel::Serializable, |tx| async {
    // ...
}).await?;
```

#### 7.6 Query Logging & Debugging

| Feature | Description | Status |
|---------|-------------|--------|
| **Query Logging** | Log all queries with timing | ✅ Implemented |
| **Slow Query Alerts** | Warn on queries exceeding threshold | 🔲 Planned |
| **Query Plan Viewer** | EXPLAIN output for SQL providers | 🔲 Planned |
| **Debug Mode** | Pretty-print queries and parameters | 🔲 Planned |

**Example:**
```rust
// Enable query logging
Repository::builder()
    .provider(provider)
    .log_queries(LogLevel::Debug)
    .slow_query_threshold(Duration::from_millis(100))
    .build();
```

#### 7.7 Auto-Generated Migrations

| Feature | Description | Status |
|---------|-------------|--------|
| **Diff-based Migration** | Generate migration from entity changes | 🔲 Planned |
| **Migration Rollback** | `migrate down` to revert changes | 🔲 Planned |
| **Migration Status** | Track which migrations applied | 🔲 Planned |

**Example:**
```rust
// Generate migration from entity changes
repo.diff_migration::<User>().await?;

// Run migrations
repo.migrate_up().await?;

// Rollback last migration
repo.migrate_down().await?;
```

#### 7.8 Global Filters (Multi-tenancy)

| Feature | Description | Status |
|---------|-------------|--------|
| **Tenant Isolation** | Automatic filter by tenant_id | 🔲 Planned |
| **Soft Delete Global Filter** | Default exclude deleted | 🔲 Planned |
| **Custom Global Scopes** | Apply filters to all queries | 🔲 Planned |

**Example:**
```rust
// Global tenant filter
repo.add_global_filter("tenant_id", tenant_id);

// All queries automatically filtered
repo.find_all().await?; // WHERE tenant_id = '...'

// Override for admin
repo.find_all().without_global_filters().await?;
```

### Lower Priority (Nice to Have)

| Feature | Description | Status |
|---------|-------------|--------|
| **Seed Data Management** | Load test/fixture data | 🔲 Planned |
| **Soft Delete Restore** | Un-delete functionality | 🔲 Planned |
| **Audit Log** | Track all changes to entities | 🔲 Planned |
| **Query Memoization** | Cache repeated identical queries | 🔲 Planned |
| **Entity Cloning** | Deep clone entity with new ID | 🔲 Planned |
| **Pagination Metadata** | Return total count, has_next, has_prev | 🔲 Planned |

---

## 8. Version Roadmap

| Version | Focus | Status |
|---------|-------|--------|
| 0.2.0 | Transactions + Pooling | ✅ |
| 0.3.0 | Soft Deletes + Validators + NoSQL Indexes | ✅ |
| 0.4.0 | Field Projection | ✅ |
| 0.5.0 | Migration System + CLI | ✅ |
| 0.6.0 | SQL Providers (PostgreSQL, SQLite, MySQL) | ✅ |
| 0.7.0 | Batch Relation Loading (RelationLoader) | ✅ |
| 0.8.0 | Query Builder Enhancements + Bulk Operations | ✅ |
| **0.9.0** | **Transaction Improvements + Global Filters** | 🔲 Planned |
| 1.0.0 | Stable API + Docs | 🔲 Planned |

---

## 9. Implementation Tasks

### SQL Infrastructure (Completed ✅)
| Task | Status |
|------|--------|
| SQL feature flags | ✅ |
| SQL types (SqlDialect, SqlColumnType, etc.) | ✅ |
| PostgresProvider | ✅ |
| SqliteProvider | ✅ |
| MySqlProvider | ✅ |
| SqlQueryBuilder | ✅ |

### Testing (In Progress)
| Task | Status |
|------|--------|
| Unit tests for SQL query generation | ✅ |
| Integration tests for PostgreSQL | 🔲 Planned |
| Integration tests for SQLite | 🔲 Planned |
| Integration tests for MySQL | 🔲 Planned |

### High Priority Remaining
| Task | Priority | Status |
|------|----------|--------|
| Chainable query builder methods | High | ✅ Implemented |
| Bulk insert/update/delete | High | ✅ Implemented |
| Cursor-based pagination | High | ✅ Implemented |
| Advanced filter operators (LIKE, BETWEEN, IN) | High | ✅ Implemented |
| Complex OR/AND filter groups | High | ✅ Implemented |
| Auto-timestamp updates | Medium | ✅ Implemented |
| Query logging | Medium | ✅ Implemented |

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

## 12. Usage Examples

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

---

## 13. Competitive Analysis

### vs TypeORM (Node.js)

| TypeORM Feature | nosql_orm Status |
|-----------------|------------------|
| Active record vs Data mapper | ✅ Data mapper done |
| Relations (one-to-many, etc.) | ✅ Done |
| Migrations | ✅ Done |
| Transaction support | ✅ Done |
| **Query builder** | 🔲 Planned |
| **Bulk operations** | 🔲 Planned |
| **Change tracking** | 🔲 Planned |

### vs Prisma (Node.js)

| Prisma Feature | nosql_orm Status |
|---------------|------------------|
| Type-safe client | ✅ Done |
| Auto-generated migrations | 🔲 Planned |
| Prisma schema | Our entity macros similar |
| **Connection pooling** | ✅ Done |
| **Transactional batching** | 🔲 Planned |

### vs SQLAlchemy (Python)

| SQLAlchemy Feature | nosql_orm Status |
|-------------------|------------------|
| ORM Expression language | 🔲 Planned |
| Core query builder | ✅ Basic done |
| Session management | ✅ Done |
| **Eager loading** | ✅ Done |
| **Lazy loading** | ✅ Done |
| **Compiled SQL** | 🔲 Planned |

---

## 14. Recommendations for Next Implementation

### Priority 1: Query Builder Enhancements

The current query system works but lacks chainable methods and advanced filters. This affects usability:

```rust
// Current approach (works but verbose)
let filter = Filter::And(vec![
    Filter::Eq("age".to_string(), json!(18)),
    Filter::Eq("status".to_string(), json!("active"))
]);
repo.find_many("users", Some(&filter), ...)

// Improved approach (planned)
repo.query()
    .where("age").gte(18)
    .and("status").eq("active")
    .find("users")
    .await?;
```

### Priority 2: Bulk Operations

Many applications need to insert/update hundreds of records efficiently:

```rust
// Insert 1000 users efficiently
repo.insert_many(users_vec).await?;

// Update all matching records
repo.update_many("users", filter, updates).await?;
```