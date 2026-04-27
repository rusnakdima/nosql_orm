# nosql_orm Features

This directory contains detailed documentation for all nosql_orm features.

## Feature List

| Feature | File | Description |
|---------|------|-------------|
| Macros | [07a-macros.md](07a-macros.md) | `#[derive(Model)]` and `#[derive(Validate)]` macros |
| Validators | [07b-validators.md](07b-validators.md) | Entity validation |
| Soft Delete | [07c-soft-delete.md](07c-soft-delete.md) | Soft delete support |
| Timestamps | [07d-timestamps.md](07d-timestamps.md) | Auto created_at/updated_at |
| Migrations | [07e-migrations.md](07e-migrations.md) | Database migrations |
| Indexes | [07f-indexes.md](07f-indexes.md) | NoSQL indexes |
| Events | [07g-events.md](07g-events.md) | Entity event listeners |
| Cascade | [07h-cascade.md](07h-cascade.md) | Cascade delete |
| Lazy | [07i-lazy.md](07i-lazy.md) | Lazy loading |
| Embedded | [07j-embedded.md](07j-embedded.md) | Embedded entities |
| Inheritance | [07k-inheritance.md](07k-inheritance.md) | Table inheritance |
| Search | [07l-search.md](07l-search.md) | Full-text search |
| Aggregation | [07m-aggregation.md](07m-aggregation.md) | Aggregation pipeline |
| CDC | [07n-cdc.md](07n-cdc.md) | Change Data Capture |
| GraphQL | [07o-graphql.md](07o-graphql.md) | GraphQL integration |
| Subscription | [07p-subscription.md](07p-subscription.md) | Pub/Sub |
| Cache | [07q-cache.md](07q-cache.md) | Query caching |
| SQL | [07r-sql.md](07r-sql.md) | SQL utilities |
| Logging | [07s-logging.md](07s-logging.md) | Query logging |

---

## Quick Feature Overview

### Core (Required for Basic Usage)
- **Macros** - Derive macros for Entity implementation
- **Entity** - Entity trait and metadata (see [02-entity.md](../02-entity.md))
- **Repository** - CRUD operations (see [04-repository.md](../04-repository.md))

### Common Features
- **Soft Delete** - Soft delete with restore
- **Timestamps** - Auto created_at/updated_at
- **Validators** - Field validation
- **Migrations** - Schema migrations
- **Indexes** - NoSQL index management

### Advanced Features
- **Relations** - Relation loading (see [06-relations.md](../06-relations.md))
- **Cascade** - Cascade delete
- **Lazy** - Lazy loading
- **Embedded** - Embedded entities
- **Inheritance** - Table inheritance

### Integrated Features
- **Search** - Full-text search
- **Aggregation** - Aggregation pipeline
- **CDC** - Change Data Capture
- **GraphQL** - GraphQL integration
- **Subscription** - Pub/Sub
- **Cache** - Query caching
- **Events** - Event listeners
- **SQL** - SQL utilities
- **Logging** - Query logging

---

## Feature Dependencies

```
Entity Trait
    │
     ├── Macros
     │
     ├── Repository
     │    │
     │    ├── Soft Delete
     │    ├── Timestamps
     │    ├── Validators
     │    ├── Migrations
     │    │
     │    └── Relations
     │         │
     │         ├── Cascade
     │         ├── Lazy
     │         └── Embedded
     │
     └── Providers (JSON, MongoDB, Redis, SQL)
          │
          ├── Indexes
          ├── Search
          ├── Aggregation
          ├── CDC
          │
          └── Logging
```

---

## Getting Started with Features

1. Start with [01-introduction.md](../01-introduction.md)
2. Learn about [Entity](../02-entity.md)
3. Configure a [Provider](../03-provider.md)
4. Use [Repository](../04-repository.md) for CRUD
5. Build [Queries](../05-query-builder.md)
6. Set up [Relations](../06-relations.md)
7. Add features as needed

---

## Feature Flags

Features are enabled via Cargo feature flags in `Cargo.toml`:

```toml
[features]
default = ["json"]
json = []                           # JSON provider
mongo = ["dep:mongodb", ...]       # MongoDB provider
redis = ["dep:redis"]               # Redis provider
full = ["json", "mongo", "redis"]   # All NoSQL providers

query_cache = []                    # Query caching

# SQL Providers
sql-postgres = [...]
sql-sqlite = [...]
sql-mysql = [...]
sql = ["sql-postgres", "sql-sqlite", "sql-mysql"]
```

---

## Related Documentation

- [../02-entity.md](../02-entity.md) - Entity system
- [../03-provider.md](../03-provider.md) - Providers
- [../04-repository.md](../04-repository.md) - Repository
- [../05-query-builder.md](../05-query-builder.md) - Query builder
- [../06-relations.md](../06-relations.md) - Relations
- [../08-examples.md](../08-examples.md) - Examples
- [../09-api-reference.md](../09-api-reference.md) - API reference