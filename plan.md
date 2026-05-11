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
| ClickHouse Provider | 🔲 Interface Only | Placeholder for future | SQL/Columnar |
| CockroachDB Provider | 🔲 Interface Only | Placeholder for future | SQL/Distributed |
| DynamoDB Provider | 🔲 Interface Only | Placeholder for future | NoSQL/Key-Value |

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
| **Multi-tenancy / Global Filters** | ✅ |
| **Embedded Entities** | ✅ |
| **Inheritance** | ✅ |
| **NoSQL Indexes** | ✅ |
| **Batch Relation Loading** | ✅ (RelationLoader) |
| **Change Tracking / Dirty Checking** | ✅ |
| **Optimistic Locking** | ✅ |
| **Global Filters (Tenant Isolation)** | ✅ |
| **Transaction Callbacks** | ✅ |
| **Savepoints** | ✅ |
| **Isolation Levels** | ✅ |
| **Retry on Deadlock** | ✅ |

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
| **Query Result Streaming** | ✅ |
| **Raw Query Execution** | ✅ |
| **JSON Path Queries** | ✅ |
| **Slow Query Alerts** | ✅ |
| **Query Plan Viewer** | ✅ |
| **Debug Mode / Pretty-print** | ✅ |
| **Diff-based Migration** | ✅ |
| **Migration Rollback** | ✅ |
| **Migration Status Tracking** | ✅ |
| **Query Timeout** | ✅ |
| **Connection Health Checks** | ✅ |
| **Read Replicas** | ✅ |
| **Prepared Statement Caching** | ✅ |
| **Mutators & Casts** | ✅ |
| **Accessors / Computed Fields** | ✅ |
| **Q Objects (Django-style)** | ✅ |
| **SQL Expression Language** | ✅ |
| **ETL Pipelines** | ✅ |
| **Schema Evolution** | ✅ |
| **Import/Export (CSV, JSON)** | ✅ |
| **Data Replication** | ✅ |
| **Field-Level Encryption** | ✅ |
| **Audit Trail** | ✅ |
| **Row-Level Security** | ✅ |
| **Query Allowlisting** | ✅ |
| **OpenTelemetry Integration** | ✅ |
| **Prometheus Metrics** | ✅ |
| **Circuit Breaker** | ✅ |
| **Rate Limiting** | ✅ |

---

## 4. Security Audit (May 2026) ✅ Fixed

All critical security vulnerabilities have been identified and fixed.

### Critical Issues Fixed

| Issue | Severity | Status |
|------|----------|--------|
| **Memory leak in PooledJson** - permit dropped immediately, pool broken | CRITICAL | ✅ Fixed |
| **Static registry race conditions** - TOCTOU and lock inversion | CRITICAL | ✅ Fixed |
| **SQL injection vulnerability** - inadequate string escaping | CRITICAL | ✅ Fixed |
| **Cache layer race conditions** - multiple locks causing deadlocks | HIGH | ✅ Fixed |
| **Event listener data race** - no synchronization on listeners Vec | HIGH | ✅ Fixed |
| **Cascade operations without transaction safety** - partial updates possible | HIGH | ✅ Fixed |
| **Blocking Mutex in async context** - deadlock potential in 5 providers | HIGH | ✅ Fixed |
| **Unbounded recursion in relation loading** - stack overflow on deep graphs | HIGH | ✅ Fixed |
| **Regex DoS (ReDoS) in PatternValidator** - catastrophic backtracking | HIGH | ✅ Fixed |
| **Missing Validate bound on insert_many** - unvalidated entities inserted | HIGH | ✅ Fixed |

### Files Modified During Security Audit

```
src/pool/pool_impl.rs          - Pool permit storage fix
src/relations/registry.rs      - Thread-safe registry with OnceLock
src/sql/query/builder.rs       - Parameterized queries
src/sql/query/filter.rs        - filter_to_sql_param method
src/cache/query_cache.rs       - Single mutex for all state
src/events/listener.rs         - tokio::sync::RwLock for listeners
src/cascade/mod.rs             - Transaction-safe cascade operations
src/relations/loader/nested.rs - max_depth parameter added
src/relations/loader/recursive.rs - max_depth parameter added
src/validators/validators_impl.rs - Regex size_limit and input length check
src/providers/json.rs          - tokio::sync::Mutex for transaction_manager
src/providers/sql/postgres.rs  - Parameterized queries, tokio::sync::Mutex
src/providers/sql/mysql.rs     - Parameterized queries, tokio::sync::Mutex, delete fix
src/providers/sql/sqlite.rs    - tokio::sync::Mutex for transaction_manager
src/providers/redis.rs         - tokio::sync::Mutex for transaction_manager
src/providers/mongo/mod.rs     - tokio::sync::Mutex for transaction_manager
src/repository/crud.rs         - Validate bound on insert_many
src/repository/delete.rs       - TransactionControl bounds added
src/lazy/lazy_relation.rs      - Proper cache access error handling
src/transaction.rs              - Poison-safe mutex handling
src/schema/prefix.rs           - Poison-safe RwLock handling
```

---

## 5. SQL Database Support (Implemented ✅)

### Providers Implemented

| Provider | File | Status |
|----------|------|--------|
| PostgreSQL | `src/providers/sql/postgres.rs` | ✅ |
| SQLite | `src/providers/sql/sqlite.rs` | ✅ |
| MySQL | `src/providers/sql/mysql.rs` | ✅ |
| ClickHouse | `src/providers/clickhouse.rs` | 🔲 Interface Only |
| CockroachDB | `src/providers/cockroach.rs` | 🔲 Interface Only |

---

## 6. Relation Loading (Implemented ✅)

### RelationLoader

Batch loading with soft-delete filtering support. Recursion depth limited to prevent stack overflow.

---

## 7. Query Builder Features

### Implemented Features

| Feature | File | Status |
|---------|------|--------|
| **Chainable Query Methods** | `src/query/builder.rs` | ✅ |
| **Complex OR/AND Filters** | `src/query/filter.rs` | ✅ |
| **Cursor-based Pagination** | `src/repository/query.rs` | ✅ |
| **Query Result Streaming** | `src/repository/mod.rs` | ✅ |
| **Raw Query Execution** | `src/repository/mod.rs` | ✅ |
| **JSON Path Queries** | `src/query/filter.rs` | ✅ |
| **Q Objects** | `src/query/q_object.rs` | ✅ |
| **SQL Expression Language** | `src/sql/expression.rs` | ✅ |

---

## 8. Transaction Management (Implemented ✅)

| Feature | File | Status |
|---------|------|--------|
| **Basic Transactions** | `src/transaction.rs` | ✅ |
| **Transaction Callbacks** | `src/transaction.rs` | ✅ |
| **Savepoints** | `src/transaction.rs` | ✅ |
| **Isolation Levels** | `src/transaction.rs` | ✅ |
| **Retry on Deadlock** | `src/transaction.rs` | ✅ |

---

## 9. Change Tracking & Dirty Checking (Implemented ✅)

| Feature | File | Status |
|---------|------|--------|
| **ChangeSet** | `src/change_tracking.rs` | ✅ |
| **DirtyChecking trait** | `src/change_tracking.rs` | ✅ |
| **TrackedEntity wrapper** | `src/change_tracking.rs` | ✅ |
| **OptimisticLocking trait** | `src/optimistic_lock.rs` | ✅ |
| **VersionedEntity wrapper** | `src/optimistic_lock.rs` | ✅ |
| **versioned_entity! macro** | `src/optimistic_lock.rs` | ✅ |
| **Auto-timestamps** | `src/timestamps.rs` | ✅ |

---

## 10. Query Logging & Debugging (Implemented ✅)

| Feature | File | Status |
|---------|------|--------|
| **QueryLogger** | `src/logging/query_logger.rs` | ✅ |
| **Slow Query Alerts** | `src/logging/query_logger.rs` | ✅ |
| **Query Plan Viewer (EXPLAIN)** | `src/providers/sql/*.rs` | ✅ |
| **Pretty-print / Debug Mode** | `src/logging/pretty.rs` | ✅ |
| **QueryDebugInfo** | `src/logging/pretty.rs` | ✅ |

---

## 11. Migration System (Implemented ✅)

| Feature | File | Status |
|---------|------|--------|
| **Migration Runner** | `src/migrations/runner.rs` | ✅ |
| **Diff-based Migration** | `src/migrations/diff.rs` | ✅ |
| **Migration Rollback** | `src/migrations/runner.rs` | ✅ |
| **Migration Status** | `src/migrations/mod.rs` | ✅ |
| **Diff Preview** | `src/cli/migrator.rs` | ✅ |

---

## 12. Global Filters / Multi-tenancy (Implemented ✅)

| Feature | File | Status |
|---------|------|--------|
| **GlobalFilter struct** | `src/repository/global_filter.rs` | ✅ |
| **FilterScope** | `src/repository/global_filter.rs` | ✅ |
| **GlobalFilterManager** | `src/repository/global_filter.rs` | ✅ |
| **Tenant Isolation** | `src/repository/mod.rs` | ✅ |
| **Soft Delete Global Filter** | `src/repository/find.rs` | ✅ |
| **Custom Global Scopes** | `src/repository/mod.rs` | ✅ |
| **Admin Bypass (find_all_unsafe)** | `src/repository/find.rs` | ✅ |

---

## 13. Performance Optimizations (Implemented ✅)

| Feature | File | Status |
|---------|------|--------|
| **Connection Health Checks** | `src/pool/health.rs` | ✅ |
| **Read Replicas** | `src/pool/replica.rs` | ✅ |
| **Prepared Statement Cache** | `src/sql/prepared.rs` | ✅ |
| **Query Timeout** | `src/repository/mod.rs` | ✅ |

---

## 14. Observability (Implemented ✅)

| Feature | File | Status |
|---------|------|--------|
| **OpenTelemetry Integration** | `src/observability/telemetry.rs` | ✅ |
| **Prometheus Metrics** | `src/observability/metrics.rs` | ✅ |
| **Circuit Breaker** | `src/observability/circuit_breaker.rs` | ✅ |
| **Rate Limiting** | `src/observability/rate_limiter.rs` | ✅ |

---

## 15. Security Features (Implemented ✅)

| Feature | File | Status |
|---------|------|--------|
| **Field-Level Encryption (AES-256-GCM)** | `src/security/encryption.rs` | ✅ |
| **Audit Trail** | `src/security/audit.rs` | ✅ |
| **Row-Level Security** | `src/security/row_level_security.rs` | ✅ |
| **Query Allowlisting** | `src/security/query_allowlist.rs` | ✅ |

---

## 16. Data Engineering (Implemented ✅)

| Feature | File | Status |
|---------|------|--------|
| **ETL Pipelines** | `src/data_engineering/etl.rs` | ✅ |
| **Schema Evolution** | `src/data_engineering/schema_evolution.rs` | ✅ |
| **Import/Export (CSV, JSON)** | `src/data_engineering/import_export.rs` | ✅ |
| **Data Replication** | `src/data_engineering/replication.rs` | ✅ |

---

## 17. Advanced ORM Features (Implemented ✅)

| Feature | File | Status |
|---------|------|--------|
| **Mutators & Casts** | `src/entity/mutators.rs` | ✅ |
| **Accessors / Computed Fields** | `src/entity/accessors.rs` | ✅ |

---

## 18. Version Roadmap

| Version | Focus | Status |
|---------|-------|-------|
| 0.2.0 | Transactions + Pooling | ✅ |
| 0.3.0 | Soft Deletes + Validators + NoSQL Indexes | ✅ |
| 0.4.0 | Field Projection | ✅ |
| 0.5.0 | Migration System + CLI | ✅ |
| 0.6.0 | SQL Providers (PostgreSQL, SQLite, MySQL) | ✅ |
| 0.7.0 | Batch Relation Loading (RelationLoader) | ✅ |
| 0.8.0 | Query Builder Enhancements + Bulk Operations | ✅ |
| 0.9.0 | Security Audit + Critical Bug Fixes | ✅ |
| 0.10.0 | **Complete Feature Set** | ✅ |
| **1.0.0** | **Stable API + Full Docs + Benchmark Suite** | 🔲 Planned |

---

## 19. Module Structure

```
src/
├── lib.rs                     # Main library exports
├── entity.rs                  # Entity trait
├── error.rs                   # OrmError, OrmResult
├── query.rs                   # Filter, QueryBuilder, SortDirection, Projection
├── relations.rs               # RelationDef, RelationLoader, WithRelations
├── repository.rs              # Repository, RelationRepository
├── soft_delete.rs             # SoftDeletable trait
├── schema.rs                  # SchemaManager
├── change_tracking.rs         # DirtyChecking, TrackedEntity
├── optimistic_lock.rs         # OptimisticLocking, VersionedEntity
├── timestamps.rs              # Timestamps trait
├── entity/
│   ├── mutators.rs            # Mutators trait, CastType
│   └── accessors.rs           # Accessors trait, ComputedField
├── query/
│   ├── q_object.rs            # Q objects for complex queries
│   └── filter.rs              # Filter enum with JsonPath
├── sql/
│   ├── expression.rs          # SQL expression language
│   └── prepared.rs             # Prepared statement caching
├── providers/
│   ├── mod.rs                 # Provider exports, ProviderType, ProviderFactory
│   ├── json/                  # JsonProvider
│   ├── mongo/                 # MongoProvider
│   ├── redis/                 # RedisProvider
│   ├── sql/                   # PostgresProvider, SqliteProvider, MySqlProvider
│   ├── clickhouse.rs          # ClickHouse placeholder
│   ├── cockroach.rs           # CockroachDB placeholder
│   └── dynamo.rs              # DynamoDB placeholder
├── cache/                     # QueryCache (query_cache feature)
├── migrations/                # Migration system
│   ├── diff.rs                # Schema diff generation
│   └── runner.rs              # Migration runner with rollback
├── validators/                # Entity validation
├── aggregation.rs             # Aggregation pipeline
├── cdc/                       # Change Data Capture
├── graphql/                   # GraphQL integration
├── lazy/                      # Lazy loading
├── nosql_index/               # NoSQL indexes
├── pool/                      # Connection pooling
│   ├── health.rs              # Connection health checks
│   └── replica.rs             # Read replicas
├── search/                    # Full-text search
├── subscription/               # Pub/sub
├── transaction.rs             # Transaction support with callbacks, savepoints
├── logging/
│   └── pretty.rs              # Query pretty-printing
├── observability/
│   ├── telemetry.rs           # OpenTelemetry integration
│   ├── metrics.rs             # Prometheus metrics
│   ├── circuit_breaker.rs      # Circuit breaker pattern
│   └── rate_limiter.rs        # Rate limiting
├── security/
│   ├── encryption.rs          # Field-level encryption
│   ├── audit.rs               # Audit trail
│   ├── row_level_security.rs  # Row-level security
│   └── query_allowlist.rs     # Query allowlisting
├── data_engineering/
│   ├── etl.rs                 # ETL pipelines
│   ├── schema_evolution.rs    # Schema evolution
│   ├── import_export.rs       # Import/export
│   └── replication.rs         # Data replication
└── repository/
    ├── global_filter.rs       # Global filters, tenant isolation
    ├── crud.rs                # Create, update, save
    ├── delete.rs              # Delete operations
    ├── find.rs                # Find operations
    ├── query.rs               # Query builder
    └── relations.rs           # Relation operations
```

---

## 20. Usage Examples

```rust
// JSON (default)
let provider = JsonProvider::new("./data").await?;

// PostgreSQL
let provider = PostgresProvider::connect("postgres://user:pass@localhost/db").await?;

// With Connection Pool
let pool = JsonPool::with_config("./data".into(), PoolConfig::new(10)).await?;
let pooled_provider = pool.acquire(true).await?;

// With Global Filters (Multi-tenancy)
let repo = Repository::new(provider)
    .with_global_filter("tenant_id", "tenant_123".into());

// With Change Tracking
let tracked = TrackedEntity::new(entity);
let changes = tracked.get_changes();

// With Transaction Callback
repo.with_transaction(|tx| async {
    tx.save(user).await?;
    tx.insert(Order { user_id: user.id.clone(), .. }).await?;
    Ok(())
}).await?;

// With Retry on Deadlock
let config = RetryConfig::default();
with_retry(config, || async {
    repo.save(entity).await
}).await?;

// With Slow Query Alerts
QueryLogger::new()
    .with_slow_query_threshold(100)
    .on_slow_query(|info| println!("Slow query: {:?}", info));

// With Query Debug
let debug = QueryDebugInfo::new()
    .with_sql("SELECT * FROM users WHERE id = $1")
    .with_params(&[json!(1)])
    .format();

// With ETL Pipeline
let pipeline = EtlPipeline::new(source_repo, transformer, dest_repo);
let stats = pipeline.run(1000).await?;

// With Audit Trail
let audit = AuditLogger::new(provider.clone(), "audit");
audit.log(AuditEntry { action: AuditAction::Update, ... }).await?;

// With Field Encryption
let encryption = FieldEncryption::new(key_base64)?;
let encrypted = entity.encrypt_fields(&encryption, &["ssn", "credit_card"])?;

// With Migration Diff Preview
let diff = generate_migration::<User>(&schema_introspection, &entity_meta)?;
println!("{}", diff.preview_sql());
```

---

## 21. Feature Flags

```toml
[features]
default = ["json"]
json = []
mongo = ["dep:mongodb", "dep:futures-util"]
redis = ["dep:redis"]
full = ["json", "mongo", "redis"]
query_cache = []
validators = []
opentelemetry = ["dep:opentelemetry"]

# SQL Providers
sql-postgres = ["dep:tokio-postgres", "dep:deadpool-postgres", "dep:base64"]
sql-sqlite = ["dep:rusqlite", "dep:base64"]
sql-mysql = ["dep:mysql_async", "dep:base64"]
sql = ["sql-postgres", "sql-sqlite", "sql-mysql"]
```

---

## 22. Contributing Goal

**Mission:** Build the most capable, ergonomic, and high-performance ORM in the Rust ecosystem that surpasses TypeORM, Prisma, Django ORM, and SQLAlchemy in features while maintaining the best developer experience.

**Key Differentiators:**
1. ✅ Single API for NoSQL + SQL databases
2. ✅ Zero-config embedded storage option
3. ✅ Native async runtime support
4. ✅ Built-in GraphQL, Pub/Sub, CDC
5. ✅ Type-safe query builder
6. ✅ Security-hardened (SQL injection, ReDoS, async-mutex)
7. ✅ Change tracking with dirty checking
8. ✅ Global filters and multi-tenancy
9. ✅ Transaction callbacks and savepoints
10. ✅ Observability (OpenTelemetry, Prometheus, Circuit Breaker)
11. ✅ ETL pipelines and data engineering
12. 🔲 Sub-millisecond query execution
13. 🔲 Auto-scaling connection management
14. 🔲 Intelligent query optimization hints
15. 🔲 Natural language query interface (AI-assisted)

---

## 23. Known Limitations

### Pre-1.0 Limitations

| Limitation | Description | Workaround |
|------------|-------------|------------|
| **No auto-generated migrations** | Must write migrations manually | Use CLI diff command |
| **No query result streaming** | All results loaded to memory | Use pagination |
| **ClickHouse/CockroachDB/DynamoDB** | Interface only, not implemented | Use PostgreSQL instead |

---

## 24. Testing Strategy

| Test Type | Coverage | Status |
|-----------|----------|--------|
| Unit tests - SQL query generation | High | ✅ |
| Unit tests - Filter builders | High | ✅ |
| Unit tests - Validation | High | ✅ |
| Unit tests - Cascade operations | Medium | ✅ |
| Unit tests - Change tracking | High | ✅ |
| Integration tests - JSON provider | High | ✅ |
| Integration tests - PostgreSQL | 🔲 Planned |
| Integration tests - SQLite | 🔲 Planned |
| Integration tests - MySQL | 🔲 Planned |
| Performance benchmarks | 🔲 Planned |