# Migrations

Database schema migration system.

> **Status:** implemented — migration bodies really execute (0.10.0+).
> `MigrationRunner` records applied versions *and* runs the statement or
> script behind each migration; `rollback` undoes both the body and the
> record.

---

## Table of Contents

1. [Migration Trait](#migration-trait)
2. [MigrationRunner](#migrationrunner)
3. [Migration Types](#migration-types)
4. [How a record is stored](#how-a-record-is-stored)

---

## Migration Trait

```rust
pub trait Migration<P: Storage>: Send + Sync {
    fn version(&self) -> i64;
    fn name(&self) -> &str;
    async fn up(&self, provider: &P) -> OrmResult<()>;
    async fn down(&self, provider: &P) -> OrmResult<()>;
}
```

The trait itself only needs `Storage`, so a custom migration can target a
provider with no query language. `SqlMigration` and `JsonMigration` add an
`AdminCommands` bound on their own impls — see below.

---

## MigrationRunner

```rust
pub struct MigrationRunner<P: Storage> { /* provider, migrations */ }

impl<P: Storage + AdminCommands> MigrationRunner<P> {
    pub fn new(provider: P) -> Self;
    pub fn add_migration<M: Migration<P> + 'static>(&mut self, migration: M);
    pub async fn run_all_pending(&self) -> OrmResult<Vec<MigrationMeta>>;
    pub async fn rollback(&self, count: u32) -> OrmResult<()>;
    pub async fn status(&self) -> OrmResult<Vec<MigrationMeta>>;
}
```

`AdminCommands` is required because the runner has to create the
`_migrations` collection: document stores make one on first write, but a SQL
provider needs its table to exist before anything can be inserted.

### MigrationMeta

```rust
pub struct MigrationMeta {
    pub version: i64,
    pub name: String,
    pub applied_at: Option<DateTime<Utc>>,
}
```

### Behaviour

- `run_all_pending` runs each un-applied migration's `up`, **then** records
  it. A migration that fails is never recorded, so it can be retried.
- `rollback(count)` runs `down` on the last `count` applied migrations and
  **deletes their records**. Without the delete a rolled-back migration
  stayed marked as applied and `run_all_pending` would never run it again.
- `rollback` errors if a migration it is asked to undo is not registered —
  silently skipping would leave the record behind.
- `status()` lists applied migrations only; the `_schema` bookkeeping row is
  filtered out rather than showing up as a phantom version 0.

---

## Migration Types

### SqlMigration

```rust
pub struct SqlMigration {
    version: i64,
    name: String,
    up_sql: String,
    down_sql: String,
}

impl SqlMigration {
    pub fn new(version: i64, name: &str, up_sql: &str, down_sql: &str) -> Self;
}
```

`up`/`down` execute `up_sql`/`down_sql` through
`AdminCommands::execute_raw`. An empty statement is a legitimate no-op (a
one-directional migration) and is skipped rather than sent to be rejected.

Because the impl requires `P: Storage + AdminCommands`, this type does not
exist for a provider that cannot run SQL; against one that implements
`AdminCommands` but has no query language (the JSON provider) the call
answers `NotSupported` at run time instead of silently doing nothing.

`SqlFileLoader` parses `--- DOWN ---`-separated `.sql` files into these, and
they now actually run.

### JsonMigration

```rust
pub struct JsonMigration {
    version: i64,
    name: String,
    up_script: Value,
    down_script: Value,
}
```

A script is a **JSON array of operations**, executed in order:

```json
[
  {"op": "create_collection", "collection": "audit"},
  {"op": "insert", "collection": "settings",
   "doc": {"id": "app", "theme": "dark"}},
  {"op": "patch", "collection": "settings", "id": "app",
   "patch": [{"op": "add", "path": "/locale", "value": "en-US"}]},
  {"op": "update", "collection": "settings", "id": "app",
   "doc": {"id": "app", "theme": "light"}},
  {"op": "delete", "collection": "settings", "id": "app"},
  {"op": "drop_collection", "collection": "audit"}
]
```

| op | effect |
|---|---|
| `create_collection` / `drop_collection` | `AdminCommands` |
| `insert` / `update` / `delete` | `Storage` |
| `patch` | reads the document, applies an RFC 6902 subset (`add`, `remove`, `replace`) to it, writes it back |

`down_script` uses the same format — the caller states how to undo, because
only the caller knows what the change was.

---

## How a record is stored

A migration record's `id` **is** the serialised `MigrationMeta`:

```json
{"id": "{\"version\":1,\"name\":\"seed_settings\",\"applied_at\":\"2026-11-16T…\"}"}
```

This is not a stylistic choice. `create_collection` guarantees an `id`
column on every provider, but the SQL providers shape it as
`(id TEXT PRIMARY KEY)` only — there is nowhere for `version`, `name` and
`applied_at` to live as their own columns without dialect-specific DDL
(`IF NOT EXISTS` and `TEXT` are not portable to Oracle, for one). Encoding
the meta in the id makes the bookkeeping work on every backend.

Records written in the older field-per-column layout are still read, so an
existing `_migrations` collection keeps working.

---

## Example

```rust
use nosql_orm::prelude::*;
use nosql_orm::{JsonMigration, MigrationRunner};

#[tokio::main]
async fn main() -> OrmResult<()> {
    let provider = JsonProvider::new("./data").await?;
    let mut runner = MigrationRunner::new(provider.clone());

    runner.add_migration(JsonMigration::new(
        1,
        "seed_settings",
        serde_json::json!([
            {"op": "create_collection", "collection": "settings"},
            {"op": "insert", "collection": "settings",
             "doc": {"id": "app", "theme": "dark"}}
        ]),
        serde_json::json!([
            {"op": "delete", "collection": "settings", "id": "app"}
        ]),
    ));

    let applied = runner.run_all_pending().await?;
    println!("Applied {} migration(s)", applied.len());

    runner.rollback(1).await?;   // undoes the body AND the record
    Ok(())
}
```

Runnable version: `cargo run --example migrations_example`.

---

## Tests

- `tests/test_migrations.rs` — runner bookkeeping on the JSON provider
  (apply, skip-already-applied, rollback-then-reapply, status excluding the
  `_schema` marker).
- `tests/test_migration_sql.rs` (gated on `sql-sqlite`) — proves the bodies
  really execute: the table appears on `up` and is gone after `down`, empty
  statements are skipped, a bad statement errors, rollback frees the record
  so the migration re-runs, and an unregistered rollback is rejected.

```bash
cargo test --features sql-sqlite --test test_migration_sql
```

---

## Next Steps

- [07f-indexes.md](07f-indexes.md) - NoSQL indexes
