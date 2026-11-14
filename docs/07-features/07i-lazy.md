# Lazy Loading

Deferred loading of relations on demand.

Status: ✅ wired into `Repository` and `RelationLoader`.
Opt-in: compiled only with the `lazy` feature.

---

## Types

```rust
pub struct Lazy<T> { ... }             // generic memoized async loader
pub struct LazyRelation<E, P> { ... }  // one entity, loaded by id on access
pub struct LazyMany<E, P> { ... }      // a collection, loaded on access
pub struct LazyLoader;                 // factory over Arc<Repository<E, P>>
```

`Lazy::new(loader)`; `Lazy::get()`, `is_loaded()`, `reload()`.
`LazyRelation`/`LazyMany` cache after the first `get()`; `close()` drops the
cache, `is_loaded()` reports whether it is warm.

## Wiring

Handles are built from a repository (or a `RelationLoader`) and resolve the
relation definition by name — an unknown name or a relation of the wrong
shape returns an `InvalidQuery` error:

```rust
// Repository side (E = the entity that DECLARES the relation)
repo.lazy_load::<User>(&post.to_value()?, "author")?     // ManyToOne/OneToOne → LazyRelation<User>
repo.lazy_load_many::<Post>(&user.to_value()?, "posts")? // OneToMany → LazyMany<Post>
repo.lazy_find(user_id)?                                  // plain lazy find_by_id on this repo

// RelationLoader side (same logic, takes a RelationDef directly)
loader.lazy_one::<User>(&post.to_value()?, &relation_def)?
loader.lazy_many::<Post>(&user.to_value()?, &relation_def)?

// Low-level constructors (unchanged)
Arc::new(repo).lazy_relation(relation_def, local_id)     // RepositoryLazyExt
Arc::new(repo).lazy_many(relation_def, local_id)
LazyLoader::relation(repo_arc, relation_def, local_id)
```

Usage:

```rust
let handle = repo.lazy_load::<User>(&post.to_value()?, "author")?;
assert!(!handle.is_loaded().await);
let author = handle.get().await?;   // first call hits Storage
assert!(handle.is_loaded().await);  // later calls return the cache
```

For `ManyToMany`, the target ids are read from the local document's
`join_field` and fetched with an `In("id", …)` filter; without a join field
the handle reports `InvalidQuery`.

`LazyRelation`/`LazyMany` hold a strong `Arc<Repository>`, so a handle stays
usable after the builder's own reference is dropped.

## Tests

`tests/test_lazy.rs` (8 tests, `#[cfg(feature = "lazy")]`): single/collection
load-on-access, join-field `ManyToMany`, caching via `lazy_find`, the
`RelationLoader` builders, shape errors, memoized `Lazy<T>`, and the
`RepositoryLazyExt` constructors.
