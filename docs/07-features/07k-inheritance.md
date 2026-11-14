# Table Inheritance

Single-table and parent-table reads scoped by a discriminator column.

Status: ✅ wired into `Repository`.
Opt-in: compiled only with the `inheritance` feature.

---

## Types

```rust
pub enum InheritanceType { SingleTable, ClassTable, ConcreteTable }

pub trait Inheritance: Send + Sync {
  fn inheritance_type() -> Option<InheritanceType>;
  fn discriminator_value() -> Option<DiscriminatorValue>;
  fn discriminator_column() -> &'static str;   // default: "type"
  fn parent_entity() -> Option<&'static str>;
  fn child_entities() -> Vec<&'static str>;
}

pub struct Discriminator { pub column: String, pub values: HashMap<String, String> }
```

`entity_inheritance!(parent, Person)` and
`entity_inheritance!(sti, Person, Student, "student")` generate the impls
(the macro leaves `discriminator_column` at its `"type"` default; write the
trait by hand to pick another column).

The discriminator is an ordinary entity field — set it when constructing the
row (e.g. `#[serde(rename = "type")] kind: String`).

## Wiring (`Repository<E>` where `E: Entity + Inheritance`)

```rust
student_repo.query_scoped()          // own table + Eq(discriminator_column, discriminator_value)
student_repo.find_all_scoped()
student_repo.find_by_id_scoped(id)   // None when the row's discriminator says otherwise
student_repo.query_in_parent("people")        // same scoping, explicit parent table
student_repo.find_all_in_parent("people")     // parent-table read for a child type
person_repo.find_all_in_parent("people")      // parent has no discriminator → whole table
student_repo.find_all_grouped_in_parent("people", &discriminator)?
// → HashMap<entity_type, Vec<Value>> — one read, rows grouped by type
```

`query_in_parent` works through `RepositoryQuery::with_table`, so the whole
builder surface (`where_eq`, `order_by`, `limit`, `count`, …) applies to the
parent table.

## Known gaps

- No joined (CTI) child loads: parent rows are returned as the entity type
  the repository is typed with; `find_all_grouped_in_parent` is the escape
  hatch when one read must be split by type.
- `parent_entity()` returns the entity *name*, not a table name — the parent
  table is always passed explicitly.

## Tests

`tests/test_inheritance.rs` (5 tests, `#[cfg(feature = "inheritance")]`):
STI scoping on the shared table, `find_by_id_scoped` rejecting wrong-type
rows, parent-table reads into a different table, grouped parent reads via
`Discriminator`, and the metadata reported by the trait.
