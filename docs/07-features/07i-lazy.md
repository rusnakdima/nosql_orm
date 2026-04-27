# Lazy Loading

Deferred loading of relations on demand.

---

## Lazy Types

```rust
pub struct Lazy<T: Entity>(Option<T>);
pub struct LazyMany<T: Entity>(Vec<T>);
pub struct LazyRelation;
pub struct LazyLoader<P: DatabaseProvider> { provider: P }
```

## Usage

```rust
let user: Lazy<User> = Lazy::new(|| async {
    repo.find_by_id("user-id").await?.unwrap()
});

let posts: LazyMany<Post> = LazyMany::new(|| async {
    query.where_eq("author_id", user_id).find().await
});
```