# Soft Delete

Virtual deletion that marks records as deleted without removing them from the database.

---

## Table of Contents

1. [SoftDeletable Trait](#softdeletable-trait)
2. [Repository Methods](#repository-methods)
3. [Query Behavior](#query-behavior)

---

## SoftDeletable Trait

```rust
pub trait SoftDeletable: Send + Sync {
    fn deleted_at(&self) -> Option<DateTime<Utc>>;
    fn set_deleted_at(&mut self, deleted_at: Option<DateTime<Utc>>);
    fn is_deleted(&self) -> bool {
        self.deleted_at().is_some()
    }
    fn mark_deleted(&mut self) {
        self.set_deleted_at(Some(Utc::now()));
    }
    fn restore(&mut self) {
        self.set_deleted_at(None);
    }
}
```

### Implementation

```rust
use chrono::{DateTime, Utc};
use nosql_orm::prelude::*;
use nosql_orm::SoftDeletable;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Option<String>,
    pub name: String,
    pub email: String,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Entity for User {
    fn meta() -> EntityMeta { EntityMeta::new("users") }
    fn get_id(&self) -> Option<String> { self.id.clone() }
    fn set_id(&mut self, id: String) { self.id = Some(id); }
    fn is_soft_deletable() -> bool { true }
}

impl SoftDeletable for User {
    fn deleted_at(&self) -> Option<DateTime<Utc>> { self.deleted_at }
    fn set_deleted_at(&mut self, deleted_at: Option<DateTime<Utc>>) { self.deleted_at = deleted_at; }
}
```

---

## Repository Methods

### soft_delete()

```rust
// Soft delete by id
repo.soft_delete("user-id").await?;
```

### restore()

```rust
// Restore soft-deleted
repo.restore("user-id").await?;
```

---

## Query Behavior

### find_all()

For SoftDeletable entities, excludes deleted records:

```rust
let users = repo.find_all().await?;  // Excludes deleted
```

### find_all_including_deleted()

Includes soft-deleted records:

```rust
let users = repo.find_all_including_deleted().await?;  // Includes deleted
```

### query()

Excludes deleted by default:

```rust
let users = repo.query()
    .where_eq("name", "Alice")
    .find()
    .await?;  // Excludes deleted
```

### query_including_deleted()

Includes deleted:

```rust
let users = repo.query_including_deleted()
    .where_eq("name", "Alice")
    .find()
    .await?;  // Includes deleted
```

---

## Example

```rust
use chrono::{DateTime, Utc};
use nosql_orm::prelude::*;
use nosql_orm::SoftDeletable;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftDeletableUser {
    pub id: Option<String>,
    pub name: String,
    pub email: String,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Entity for SoftDeletableUser {
    fn meta() -> EntityMeta { EntityMeta::new("users") }
    fn get_id(&self) -> Option<String> { self.id.clone() }
    fn set_id(&mut self, id: String) { self.id = Some(id); }
    fn is_soft_deletable() -> bool { true }
}

impl SoftDeletable for SoftDeletableUser {
    fn deleted_at(&self) -> Option<DateTime<Utc>> { self.deleted_at }
    fn set_deleted_at(&mut self, d: Option<DateTime<Utc>>) { self.deleted_at = d; }
}

#[tokio::main]
async fn main() -> OrmResult<()> {
    let provider = JsonProvider::new("./data").await?;
    let repo: Repository<SoftDeletableUser, _> = Repository::new(provider);

    // Create
    let user = repo.save(SoftDeletableUser {
        id: None,
        name: "Alice".into(),
        email: "alice@example.com".into(),
        deleted_at: None,
    }).await?;

    // Count (before delete)
    let count = repo.count().await?;
    println!("Count before delete: {}", count);

    // Soft delete
    repo.soft_delete(&user.id.unwrap()).await?;
    println!("Soft deleted user: {}", user.id.unwrap());

    // Count (after delete - excludes soft deleted)
    let count = repo.count().await?;
    println!("Count after delete (find_all): {}", count);

    // Include deleted
    let all = repo.find_all_including_deleted().await?;
    println!("Count with deleted: {}", all.len());

    // Restore
    repo.restore(&user.id.unwrap()).await?;
    println!("Restored user: {}", user.id.unwrap());

    // Count (restored)
    let count = repo.count().await?;
    println!("Count after restore: {}", count);

    Ok(())
}
```

---

## With Macro

```rust
#[derive(Model)]
#[table_name("users")]
#[soft_delete]
pub struct User {
    pub id: Option<String>,
    pub name: String,
    pub email: String,
}
```

---

## Next Steps

- [07d-timestamps.md](07d-timestamps.md) - Auto timestamps