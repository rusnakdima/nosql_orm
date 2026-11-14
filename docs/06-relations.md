# Relations

`nosql_orm` models relations as `RelationDef` descriptors plus a loader.
Relations are declared on entities via `WithRelations`.

---

## Relation types

```rust
pub enum RelationType { OneToOne, OneToMany, ManyToOne, ManyToMany }
```

`RelationDef` constructors:

```rust
RelationDef::many_to_one(name, target_collection, local_key)
RelationDef::one_to_many(name, target_collection, foreign_key)
RelationDef::one_to_one(name, target_collection, local_key)
RelationDef::many_to_many(name, target_collection, join_field)
```

Builder methods on `RelationDef`:

- `transform_map(lookup_key, source_collection, source_key)`
- `local_key_in_array(array_field)`
- `on_delete(SqlOnDelete)` (sets cascade flags for `Cascade`/`Restrict`/...)
- `cascade_soft_delete(bool)`, `cascade_hard_delete(bool)`

Fields of `RelationDef`: `name`, `relation_type`, `target_collection`,
`local_key`, `foreign_key`, `join_field`, `local_key_in_array`,
`transform_map_via`, `on_delete`, `cascade_soft_delete`,
`cascade_hard_delete`.

---

## Declaring relations on an entity

Manual:

```rust
impl WithRelations for Post {
    fn relations() -> Vec<RelationDef> {
        vec![
            RelationDef::many_to_one("author", "users", "author_id"),
            RelationDef::many_to_many("tags", "tags", "tag_ids"),
        ]
    }
}
```

Macro (on the struct, with `#[derive(Model)]`):

```rust
#[derive(Model)]
#[many_to_one("author", "users", "author_id")]
#[many_to_many("tags", "tags", "tag_ids")]
pub struct Post { ... }
```

`Relations`/`relations` list attribute generates `one_to_many` relations
using the snake-cased collection name and `<name>_id` key.

---

## Registry

```rust
pub fn register_collection_relations(collection, relations: Vec<RelationDef>);
pub fn register_relations_for_entity<E: WithRelations>();
pub fn get_collection_relations(collection) -> Vec<RelationDef>;
pub fn get_relation_def(collection, name) -> Option<RelationDef>;
pub fn get_registered_collection_relations(collection) -> ...;
pub fn clear_relation_registry();
pub fn clear_registered_collections();
```

`register_relations_for_entity::<E>()` is the usual entry point; providers and
`CascadeManager` consult the registry.

---

## Loading relations

`RelationLoader<P>` (constructed with a provider) loads relations eagerly.
`RelationRepository` is the ergonomic wrapper:

```rust
let posts: RelationRepository<Post, _> = RelationRepository::new(provider);
let post = posts.find_with_relations(&id, &["author", "tags"]).await?.unwrap();

if let Some(author) = post.one("author")? { /* Value */ }
for tag in post.many("tags")? { /* Value */ }
```

`find_with_relations` / `find_all_with_relations` / `query_with_relations`
are only available when `E: WithRelations + SoftDeletable`.

`WithLoaded<E>`:

```rust
pub struct WithLoaded<E> { pub entity: E, pub loaded: HashMap<String, RelationValue> }
impl WithLoaded<E> {
    pub fn new(entity: E) -> Self;
    pub fn one(&self, name: &str) -> OrmResult<Option<&Value>>;
    pub fn many(&self, name: &str) -> OrmResult<&[Value]>;
    pub fn get(&self, path: &str) -> Option<&RelationValue>;
    pub fn keys(&self) -> Vec<&String>;
    pub fn has(&self, name: &str) -> bool;
}
```

`RelationValue` is `Single(Option<Value>)` or `Many(Vec<Value>)`.

### QueryBuilder relation hints

`QueryBuilder::with_relation(name)` records a relation name in the builder;
`RepositoryQuery::with_relation(name)` proxies it. These hints are stored but
the actual loading is performed via `RelationRepository`.

---

## Cascade delete

```rust
pub struct CascadeManager<P: Storage> { ... }
impl CascadeManager<P> {
    pub fn new(provider: P) -> Self;
    pub async fn check_restrict<E: Entity + WithRelations>(entity_id, relations) -> OrmResult<bool>;
    pub async fn soft_delete(collection, id) -> OrmResult<bool>;
    // hard_delete_cascade, restore, toggle_delete via submodules
}
```

`RelationDef::on_delete(SqlOnDelete::Cascade)` sets `cascade_hard_delete` and
`cascade_soft_delete` to true. `SqlOnDelete` variants: `Cascade`, `Restrict`,
`SetNull`, `SetDefault`, `NoAction`.

`Repository::delete` consults relations and dispatches to the cascade
manager when hard cascade is configured.

---

## Lazy loading (`lazy` feature)

```rust
pub struct Lazy<T> { ... }            // generic memoized loader
pub struct LazyRelation<E, P> { ... }  // single relation
pub struct LazyMany<E, P> { ... }     // collection relation
pub struct LazyLoader;                 // factory
```

`Lazy::new(loader)`; `Lazy::get()`, `is_loaded()`, `reload()`.

`RepositoryLazyExt` (implemented for `Repository<E, P>`) adds
`lazy_relation(self: Arc<Self>, relation, local_id)` and
`lazy_many(self: Arc<Self>, relation, local_id)`. `LazyMany::with_filter(filter)`
narrows the loaded set.

---

## Embedded entities (`embedded` feature)

```rust
pub trait Embedded { fn embedded_meta() -> EmbeddedMeta; }
pub struct EmbeddedMeta { prefix: Option<String>, flatten: bool }
pub trait EmbedExt { fn embed(&self, field) -> Value; fn unembed(&self, field) -> Value; }
pub struct Embedder;  // flatten / unflatten helpers
```

---

## Inheritance (`inheritance` feature)

```rust
pub enum InheritanceType { SingleTable, ClassTable, ConcreteTable }
pub trait Inheritance { inheritance_type, discriminator_value, parent_entity, child_entities }
pub struct Discriminator { column, values }
pub trait Discriminated { discriminator_column, discriminator_value }
```

`entity_inheritance!` macro generates `Inheritance` impls for STI/parent.

---

## Wrappers

```rust
pub struct OneToOne<T: Entity>(pub Option<T>);
pub struct ManyToOne<T: Entity>(pub Option<T>);
pub struct OneToMany<T: Entity>(pub Vec<T>);
pub struct ManyToMany<T: Entity>(pub Vec<T>);
```

Each has a `get()` accessor.

---

## Examples

See `examples/todo_with_all_relations.rs`, `examples/nested_relations_5_levels.rs`,
`examples/deep_relation_loading.rs`, `examples/relation_filter_example.rs`.

---

## Next steps

- [07-features/](07-features/) — Feature deep dives
- [09-api-reference.md](09-api-reference.md) — API reference