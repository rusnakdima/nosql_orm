# Examples

Runnable examples live in `examples/`. Run with:

```bash
cargo run --example json_example
cargo run --example sql_example --features sql-sqlite
cargo run --example mongo_example --features mongo
cargo run --example sql_postgres_example --features sql-postgres
cargo run --example sql_mysql_example --features sql-mysql
```

---

## Example inventory

| Example | Features | Topic |
|---|---|---|
| `json_example.rs` | `json` (default) | CRUD + relations with JSON provider |
| `mongo_example.rs` | `mongo` | MongoDB CRUD |
| `sql_example.rs` | `sql-sqlite` | SQLite CRUD |
| `sql_postgres_example.rs` | `sql-postgres` | PostgreSQL CRUD |
| `sql_mysql_example.rs` | `sql-mysql` | MySQL CRUD |
| `soft_delete_example.rs` | default | Soft delete + restore |
| `schema_example.rs` | default | Schema / SQL columns |
| `projection_example.rs` | default | Field projection |
| `migrations_example.rs` | default | Migrations |
| `index_example.rs` | default | Indexes |
| `decorator_example.rs` | default | Macros / attributes |
| `relation_filter_example.rs` | default | Filtering relations |
| `todo_tasks_subtasks.rs` | default | Nested relations |
| `todo_with_all_relations.rs` | default | All relation types |
| `nested_relations_5_levels.rs` | default | Deep nesting |
| `deep_relation_loading.rs` | default | Relation loading |
| `taskflow_like.rs` | default | TaskFlow-style usage |
| `derive_entity_test.rs`, `test_macro.rs`, `test_snake_case.rs` | default | Macro tests |

---

## Minimal end-to-end example

```rust
use nosql_orm::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Option<String>,
    pub name: String,
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

    let u = repo.save(User { id: None, name: "Alice".into() }).await?;
    println!("saved {}", u.id.unwrap());

    let all = repo.find_all().await?;
    println!("count {}", all.len());

    Ok(())
}
```

---

## Tips

- Use `tempfile::tempdir()` in examples/tests to avoid polluting disk.
- Set `NOSQL_ORM_DATA_DIR` to centralise JSON storage.
- Integration tests for Postgres/MySQL/Redis require live servers and are
  `#[ignore]`d.

---

## Next steps

- [09-api-reference.md](09-api-reference.md) — API reference
- [07-features/](07-features/) — Feature deep dives