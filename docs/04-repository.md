# Repository

`Repository<E, P>` is the high-level, typed CRUD entry point. It is generic
over the entity `E: Entity` and the provider `P: Storage`.

---

## Construction

```rust
pub struct Repository<E, P> { ... }

impl<E, P> Repository<E, P> where E: Entity, P: Storage {
    pub fn new(provider: P) -> Self;
    pub fn provider(&self) -> &P;
    pub fn with_events(self, events: Arc<EntityEvents>) -> Self;
    pub fn with_timeout(self, timeout_ms: u64) -> Self;
    pub fn indexes(&self) -> IndexManager<P>;
    pub async fn execute_sql(&self, sql: &str) -> OrmResult<usize>   // P: AdminCommands
}
```

`sync_schema(&self)` is available when `P: Storage + AdminCommands +
SqlProviderHelpers` (SQL providers) and creates the table from
`E::sql_columns()`.

`RelationRepository<E, P>` wraps a `Repository` and a `RelationLoader`:

```rust
pub struct RelationRepository<E, P> where E: WithRelations, P: Storage {
    pub fn new(provider: P) -> Self;
    pub fn repo(&self) -> &Repository<E, P>;
}
```

---

## Write operations (`crud.rs`)

| Method | Signature | Notes |
|---|---|---|
| `insert` | `async fn insert(&self, entity: E) -> OrmResult<E>` (E: Validate) | validates, mints id if missing, applies insert timestamps, dispatches insert event |
| `update` | `async fn update(&self, entity: E) -> OrmResult<E>` (E: Validate) | requires id; applies update timestamps; dispatches update event |
| `update_with_immutable` | `async fn update_with_immutable(&self, entity, immutable_fields: &[&str]) -> OrmResult<E>` | restores listed fields from the stored row |
| `save` | `async fn save(&self, entity: E) -> OrmResult<E>` (E: Validate) | upsert: insert if id is None else update |
| `insert_many` | `async fn insert_many(&self, entities: Vec<E>) -> OrmResult<usize>` | loops insert (no validation) |
| `update_many` | `async fn update_many(&self, filter, updates) -> OrmResult<usize>` | provider-level |
| `upsert_many` | `async fn upsert_many(&self, entities) -> OrmResult<usize>` (E: Validate) | loops save |
| `delete_many` | `async fn delete_many(&self, filter) -> OrmResult<usize>` | provider-level |
| `patch` | `async fn patch(&self, id, patch: Value) -> OrmResult<E>` | partial update, returns reloaded entity |
| `save_from_value` | `async fn save_from_value(&self, value: Value) -> OrmResult<E>` | upsert from a JSON value |
| `update_from_value` | `async fn update_from_value(&self, value: Value) -> OrmResult<E>` | update from a JSON value |

## Delete operations (`delete.rs`)

| Method | Notes |
|---|---|
| `delete(id)` | hard delete; consults relations / cascade |
| `remove(id)` | alias-style removal |
| `soft_delete(id)` | sets `deleted_at = now()` (E: SoftDeletable) |
| `restore(id)` | clears `deleted_at` (E: SoftDeletable) |

## Find operations (`find.rs`)

| Method | Notes |
|---|---|
| `find_by_id(id) -> OrmResult<Option<E>>` | |
| `get_by_id(id) -> OrmResult<E>` | errors with `NotFound` if absent |
| `find_all() -> OrmResult<Vec<E>>` | for soft-deletable entities this includes deleted rows |
| `find_all_including_deleted()` | explicit |
| `count() -> OrmResult<u64>` | |
| `exists(id) -> OrmResult<bool>` | |
| `find_for_frontend() -> OrmResult<Vec<E>>` (E: FrontendProjection) | applies projection excluding frontend-excluded fields |
| `find_by_id_for_frontend(id)` | same, single |
| `find_by_id_with_timeout(id)` | uses `query_timeout` (default 30s) |
| `find_all_with_timeout()` | |
| `count_with_timeout()` | |

## Querying

```rust
pub fn query(&self) -> RepositoryQuery<'_, E, P>
pub fn query_including_deleted(&self) -> RepositoryQuery<'_, E, P>
```

`RepositoryQuery` proxies a subset of `QueryBuilder` methods: `where_eq`,
`where_ne`, `where_gt`, `where_lt`, `where_contains`, `where_in`, `order_by`,
`skip`, `limit`, `with_relation`, `filter`, `select`, `exclude`. Terminal:
`find`, `find_one`, `find_with_cursor`, `find_raw`, `count`.

Streaming:

```rust
pub async fn find_stream(filter, skip, limit, sort_by, sort_asc) -> OrmResult<QueryStream<E, P>>
pub async fn find_many_stream(filter, skip, limit) -> OrmResult<QueryStream<E, P>>
```

`QueryStream` fetches in batches (default 100) via `next_item()`.

---

## Relations

`RelationRepository` adds (only when `E: WithRelations + SoftDeletable`):

```rust
pub async fn find_with_relations(id, relation_paths: &[&str]) -> OrmResult<Option<WithLoaded<E>>>
pub async fn find_all_with_relations(relation_paths) -> OrmResult<Vec<WithLoaded<E>>>
pub async fn query_with_relations(builder: QueryBuilder, relation_paths) -> OrmResult<Vec<WithLoaded<E>>>
pub async fn soft_delete_cascade(id) -> OrmResult<bool>
```

`WithLoaded<E>` exposes `entity`, `loaded: HashMap<String, RelationValue>` and
methods `one(name)`, `many(name)`, `get(path)`, `keys()`, `has(name)`. It
serializes as the entity plus the loaded relation values.

---

## Index management

```rust
self.indexes()  // IndexManager<P>
```

`IndexManager` wraps the provider's index operations. See
[07-features/07f-indexes.md](07-features/07f-indexes.md).

---

## Events

```rust
let events = Arc::new(EntityEvents::new());
let repo = Repository::new(provider).with_events(events.clone());
```

`EntityEvents` dispatches `after_insert`, `before_update`/`after_update` and
`before_delete`/`after_delete` to registered `EntityEventListener<Value>`
listeners. `dispatch_insert` is called on insert; `dispatch_update` on update.

---

## Examples

```rust
let repo: Repository<User, _> = Repository::new(provider);

// insert
let u = repo.insert(User { id: None, name: "Alice".into(), email: "a@x.com".into() }).await?;

// find
assert!(repo.find_by_id(u.id.as_ref().unwrap()).await?.is_some());

// patch
let patched = repo.patch(u.id.as_ref().unwrap(), serde_json::json!({"name":"Bob"})).await?;

// delete
repo.delete(u.id.as_ref().unwrap()).await?;
```

---

## Next steps

- [05-query-builder.md](05-query-builder.md) — Querying
- [06-relations.md](06-relations.md) — Relations
- [07-features/](07-features/) — Features