# Cascade Delete

Automatic deletion of related entities.

---

## CascadeManager

```rust
pub struct CascadeManager<P: DatabaseProvider> {
    provider: P,
}

impl<P: DatabaseProvider> CascadeManager<P> {
    pub fn new(provider: P) -> Self;
    pub async fn hard_delete_cascade<E: WithRelations>(
        &self, id: &str, relations: &[RelationDef], deleted: &mut HashSet<String>
    ) -> OrmResult<bool>;
    pub async fn soft_delete_cascade<E: WithRelations + SoftDeletable>(
        &self, id: &str, relations: &[RelationDef], deleted: &mut HashSet<String>
    ) -> OrmResult<bool>;
}
```

---

## Relation Cascade Options

```rust
let relation = RelationDef::many_to_one("author", "users", "author_id")
    .on_delete(SqlOnDelete::Cascade);
```

---

## Usage

```rust
// When relation has cascade enabled
let deleted = repo.delete("user-id").await?;  // Cascades to posts
```