# Embedded Entities

Embedded value objects stored flat inside their owner's document.

Status: ✅ wired into `Repository` (write path, read path, filters).
Opt-in: compiled only with the `embedded` feature.

---

## Declaring embedded fields

The owner entity lists its embedded field names (the serialized JSON names):

```rust
#[derive(Serialize, Deserialize, Entity)]
#[embedded(address)]                 // struct-level; #[embedded = "address, contact"] also works
pub struct User {
  pub id: Option<String>,
  pub name: String,
  pub address: Address,              // Address { street, city }
}
```

Hand-written `Entity` impls declare the same list with
`fn embedded_fields() -> Vec<&'static str>` (always available on the trait;
without the feature flag the declaration is inert and storage stays nested).

## What the wiring does

- **Write** — `Entity::to_value()` flattens declared fields before the
  provider sees them: `{ "address": { "street": "Main", "city": "Kyiv" } }`
  is stored as `address_street` / `address_city`. `insert`, `insert_many`,
  `update`, `patch`, `save_from_value` and `update_from_value` all go through
  it.
- **Read** — `Entity::from_value()` unflattens prefixed keys back into the
  nested shape, so every read path (`find_by_id`, `find_all`, `query().find`,
  streams, relation loading, `WithLoaded` serialization) returns nested
  documents again.
- **Filters** — a filter path such as `address.city` is rewritten to
  `address_city` on the way to the provider (`Filter::for_entity::<E>()`).
  This covers `RepositoryQuery::find/count/find_raw/find_with_cursor`,
  `delete_many`, `update_many` and `RelationRepository::query_with_relations`.
  The rewrite is driven by the same `embedded_fields()` list, so storage and
  filters always agree.
- **Events** — insert/update/delete event payloads carry the nested (domain)
  shape, not the flat storage shape.

Storage is idempotent: flattening an already-flat document and unflattening
an already-nested document are no-ops.

## Known gaps

- `Repository::find_stream` / `find_many_stream` pass their filter to the
  provider unchanged (the file carrying them is not writable in this
  workspace). Stream callers can adapt the filter themselves with the public
  helper: `Filter::Eq("address.city", v).for_entity::<User>()`.
- `find_raw` deliberately returns raw storage documents (flat).
- A field name that collides with a flattened prefix (`address_book` next to
  an embedded `address`) is ambiguous — rename one of them.

## Tests

`tests/test_embedded.rs` (7 tests, `#[cfg(feature = "embedded")]`): flat
storage on insert, nested reads, nested-path filters through `find`/`count`,
update + patch staying flat, `delete_many` with a nested filter, streamed
reads (with the explicit filter helper), and a plain entity proving it is
untouched.
