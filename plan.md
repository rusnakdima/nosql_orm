# nosql_orm - Implementation Plan

## 1. Library Verification

**Status: ✅ Complete**

Library properly initialized as Rust library with `src/lib.rs`, prelude exports, and feature flags.

---

## 2. Current Database Integrations

| Provider | Status | Backend |
|----------|--------|---------|
| JSON Provider | ✅ Implemented | File-based JSON storage (embedded, zero-config) |
| MongoDB Provider | ✅ Implemented | MongoDB driver v2 |
| Redis Provider | ✅ Implemented | Caching, pub/sub, sessions, streams |

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
| **NoSQL Indexes** | ⚠️ Planned | See `plan_indexes.md` |

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

## 4. Planned NoSQL Providers

### Document Stores
| Provider | Priority |
|----------|----------|
| **Elasticsearch** | High |
| **DynamoDB** | Medium |
| **CouchDB** | Low |
| **Couchbase** | Low |

### Wide-Column Stores
| Provider | Priority |
|----------|----------|
| **Apache Cassandra** | Medium |
| **ScyllaDB** | Low |

### Graph Databases
| Provider | Priority |
|----------|----------|
| **Neo4j** | High |
| **Amazon Neptune** | Medium |
| **ArangoDB** | Medium |
| **Memgraph** | Low |

### Cache Databases
| Provider | Priority |
|----------|----------|
| **Memcached** | Medium |
| **Dragonfly** | Low |

---

## 5. NoSQL Indexes

**Detailed plan: See `plan_indexes.md`**

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

## 6. Field Projection (SELECT/EXCLUDE)

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

### How It Works

1. **Projection** struct stores `select` (fields to include) or `exclude` (fields to skip)
2. Applied at repository level after fetching from provider
3. Works with all providers (JSON, MongoDB, Redis)

### Important Notes

- **All fields should be `Option<T>`** if you plan to use `select()` frequently
- Non-selected required fields will cause deserialization errors
- Use `exclude()` when you have required fields but want to hide sensitive data
- `find_raw()` returns `Value` instead of entity - no deserialization issues

---

## 7. Version Roadmap

| Version | Focus | Status |
|---------|-------|--------|
| 0.2.0 | Transactions + Pooling | ✅ |
| 0.3.0 | Soft Deletes + Validators | ✅ |
| 0.4.0 | Field Projection | ✅ |
| 0.5.0 | Migration System + CLI | ✅ |
| 0.5.1 | **NoSQL Indexes** | **Planned** |
| 0.6.0 | Elasticsearch Provider | Planned |
| 0.7.0 | Neo4j Graph Provider | Planned |
| 1.0.0 | Stable API + Docs | Planned |
