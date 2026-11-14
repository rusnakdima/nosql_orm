# nosql_orm

A TypeORM-inspired async ORM for Rust with one unified, type-safe API over
JSON files, MongoDB, Redis, PostgreSQL, SQLite and MySQL.

Crate: `nosql_orm` 0.9.0. Derive crate: `nosql_orm_derive` (re-exported).

## Features

- Unified `Storage` trait across all backends
- `Repository<E, P>` typed CRUD, queries, streaming
- `RelationRepository` with eager relation loading (`WithLoaded`)
- Fluent `QueryBuilder` + `Filter` (19 variants) + `Projection`
- Derive macros: `Model`, `OrmEntity`, `Entity`, `Validate`
- Soft delete, auto timestamps, validators, migrations (basic runner)
- Indexes (Mongo + SQL), aggregation (JSON + Mongo)
- Events, cascade delete, lazy loading, embedded, inheritance (opt-in)
- Query caching (opt-in), query logging
- Implemented: full-text search, CDC, real-time subscriptions, vector search,
  multi-tenancy, transactions (JSON, SQLite, Postgres, MySQL, Mongo, Redis),
  batch ops, streaming, offline sync, encryption, vector/semantic search
- Coming soon: new SQL providers (MSSQL, Oracle, CockroachDB, DuckDB, ClickHouse, MariaDB) — stubs added,
  new NoSQL providers (CouchDB, Couchbase, DynamoDB, Firestore, Elasticsearch, Neo4j, InfluxDB, Cassandra, etcd) — stubs added,
  GraphQL provider (stub), vector database providers (Pinecone, Weaviate, Milvus, Qdrant, Chroma, pgvector, Redis vector) — stubs added

## Installation

```toml
[dependencies]
nosql_orm = "0.9"                                          # JSON only
# nosql_orm = { version = "0.9", features = ["mongo"] }
# nosql_orm = { version = "0.9", features = ["redis"] }
# nosql_orm = { version = "0.9", features = ["sql"] }        # all SQL
# nosql_orm = { version = "0.9", features = ["full"] }
```

Feature flags: `json` (default), `mongo`, `redis`, `sql` (`sql-postgres`,
`sql-sqlite`, `sql-mysql`), `full`, `query_cache`, `embedded`,
`inheritance`, `lazy`. Search/CDC/subscriptions are documented as coming soon
(see docs/07-features/). GraphQL is not planned.

## Quick start

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
    let tmp = tempfile::tempdir().unwrap();
    let provider = JsonProvider::new(tmp.path()).await?;
    let repo: Repository<User, _> = Repository::new(provider);

    let u = repo.save(User { id: None, name: "Alice".into(), email: "a@x.com".into() }).await?;
    println!("saved {}", u.id.unwrap());

    let adults = repo.query().where_gt("age", 18).limit(10).find().await.unwrap_or_default();
    let _ = adults;
    Ok(())
}
```

## Providers

| Provider | Feature | Constructor |
|---|---|---|
| `JsonProvider` | `json` (default) | `JsonProvider::new(base_dir).await` |
| `JsonTreeProvider` | `json` | `JsonTreeProvider::new(base_dir).await` |
| `MongoProvider` | `mongo` | `MongoProvider::connect(uri, db_name).await` |
| `RedisProvider` | `redis` | `RedisProvider::new(conn_str).await` |
| `SqliteProvider` | `sql-sqlite` | `SqliteProvider::connect(path).await` |
| `PostgresProvider` | `sql-postgres` | `PostgresProvider::connect(uri).await` |
| `MySqlProvider` | `sql-mysql` | `MySqlProvider::connect(uri).await` |

Runtime selection: `connect_backend(BackendKind, url)` →
`Arc<dyn StorageBackend>`.

## Documentation

See `docs/` (table of contents: `docs/00-table-of-contents.md`):

- `01-introduction.md` — overview, feature matrix, install
- `02-entity.md` — Entity trait, macros
- `03-provider.md` — providers & feature flags
- `04-repository.md` — Repository CRUD
- `05-query-builder.md` — querying
- `06-relations.md` — relations, cascade, lazy, embedded, inheritance
- `07-features/` — feature deep dives
- `08-examples.md` — examples
- `09-api-reference.md` — API quick reference
- `ARCHITECTURE.md`, `MODULE.md`

## License

MIT.