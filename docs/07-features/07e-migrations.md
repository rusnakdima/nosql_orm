# Migrations

Database schema migration system.

---

## Table of Contents

1. [Migration Trait](#migration-trait)
2. [MigrationRunner](#migrationrunner)
3. [Migration Types](#migration-types)

---

## Migration Trait

```rust
pub trait Migration<P: DatabaseProvider>: Send + Sync {
    fn version(&self) -> i64;
    fn name(&self) -> &str;
    async fn up(&self, provider: &P) -> OrmResult<()>;
    async fn down(&self, provider: &P) -> OrmResult<()>;
}
```

---

## MigrationRunner

```rust
pub struct MigrationRunner<P: DatabaseProvider> {
    provider: P,
    migrations: Vec<Box<dyn Migration<P>>>,
}
```

### Methods

```rust
impl<P: DatabaseProvider> MigrationRunner<P> {
    pub fn new(provider: P) -> Self;
    pub fn add_migration<M: Migration<P> + 'static>(&mut self, migration: M);
    pub async fn run_all_pending(&self) -> OrmResult<Vec<MigrationMeta>>;
    pub async fn rollback(&self, count: u32) -> OrmResult<()>;
    pub async fn status(&self) -> OrmResult<Vec<MigrationMeta>>;
}
```

### MigrationMeta

```rust
pub struct MigrationMeta {
    pub version: i64,
    pub name: String,
    pub applied_at: Option<DateTime<Utc>>,
}
```

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

impl<P: DatabaseProvider> Migration<P> for SqlMigration { ... }
```

### JsonMigration

```rust
pub struct JsonMigration {
    version: i64,
    name: String,
    up_json: Value,
    down_json: Value,
}

impl JsonMigration {
    pub fn new(version: i64, name: &str, up_json: Value, down_json: Value) -> Self;
}

impl<P: DatabaseProvider> Migration<P> for JsonMigration { ... }
```

---

## Example

```rust
use nosql_orm::prelude::*;
use nosql_orm::{MigrationRunner, SqlMigration, MigrationMeta};

#[tokio::main]
async fn main() -> OrmResult<()> {
    let provider = JsonProvider::new("./data").await?;
    let runner = MigrationRunner::new(provider.clone());

    // Add migrations
    let migration1 = SqlMigration::new(
        1,
        "create_users_table",
        "CREATE TABLE users (id TEXT PRIMARY KEY, name TEXT NOT NULL);",
        "DROP TABLE users;",
    );

    let migration2 = JsonMigration::new(
        2,
        "add_email_field",
        serde_json::json!([{"op": "add", "path": "/email", "value": ""}]),
        serde_json::json!([{"op": "remove", "path": "/email"}]),
    );

    let mut runner = runner;
    runner.add_migration(migration1);
    runner.add_migration(migration2);

    // Run pending migrations
    let applied: Vec<MigrationMeta> = runner.run_all_pending().await?;
    println!("Applied {} migrations", applied.len());

    for meta in &applied {
        println!("  - v{}: {}", meta.version, meta.name);
    }

    // Check status
    let status: Vec<MigrationMeta> = runner.status().await?;
    println!("Total applied: {}", status.len());

    // Rollback
    runner.rollback(1).await?;

    Ok(())
}
```

---

## Next Steps

- [07f-indexes.md](07f-indexes.md) - NoSQL indexes