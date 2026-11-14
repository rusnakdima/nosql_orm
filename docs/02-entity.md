# Entity System

The `Entity` trait is the core abstraction representing a stored record. This
document covers manual implementations, metadata, related traits and the derive
macros.

---

## Basic implementation

```rust
use nosql_orm::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Option<String>,
    pub name: String,
    pub email: String,
}

impl Entity for User {
    fn meta() -> EntityMeta {
        EntityMeta::new("users")
    }
    fn get_id(&self) -> Option<String> { self.id.clone() }
    fn set_id(&mut self, id: String) { self.id = Some(id); }
}
```

`Entity` requires `Serialize + DeserializeOwned + Debug + Send + Sync + 'static`.

### Entity trait (actual signature)

```rust
pub trait Entity: Serialize + DeserializeOwned + Debug + Send + Sync + 'static {
    fn meta() -> EntityMeta;

    fn fields() -> Vec<FieldMeta> { Vec::new() }

    fn get_id(&self) -> Option<String>;
    fn set_id(&mut self, id: String);

    fn to_value(&self) -> OrmResult<Value> { /* serde_json::to_value */ }
    fn from_value(value: Value) -> OrmResult<Self> { /* serde_json::from_value,
        with boolean coercion via boolean_fields() */ }

    fn boolean_fields() -> Vec<&'static str> { Vec::new() }

    fn table_name() -> String { Self::meta().table_name }

    fn is_soft_deletable() -> bool { false }

    fn indexes() -> Vec<NosqlIndex> { Vec::new() }

    fn sql_columns() -> Vec<SqlColumnDef> { Vec::new() }
}
```

`from_value` runs `with_booleans(value, &Self::boolean_fields())` so that
integer 0/1 columns coming from SQL backends are coerced back to booleans.

---

## EntityMeta

```rust
pub struct EntityMeta {
    pub table_name: String,          // collection / table name
    pub id_field: String,            // primary key field, default "id"
    pub sql_columns: Option<Vec<SqlColumnDef>>,
}

impl EntityMeta {
    pub fn new(table_name: impl Into<String>) -> Self;
    pub fn with_id_field(mut self, field: impl Into<String>) -> Self;
    pub fn with_sql_columns(mut self, columns: Vec<SqlColumnDef>) -> Self;
    pub fn sql_table_name(&self) -> String;
}
```

---

## Related traits

### WithRelations

```rust
pub trait WithRelations: Entity {
    fn relations() -> Vec<RelationDef> { Vec::new() }
}
```

### SoftDeletable

```rust
pub trait SoftDeletable: Send + Sync {
    fn deleted_at(&self) -> Option<DateTime<Utc>>;
    fn set_deleted_at(&mut self, deleted_at: Option<DateTime<Utc>>);
    fn is_deleted(&self) -> bool;   // default: deleted_at().is_some()
    fn mark_deleted(&mut self);      // default: set now()
    fn restore(&mut self);           // default: set None
}
```

A `SoftDeleteExt` struct is also provided for embedding `deleted_at`.

### FrontendProjection

```rust
pub trait FrontendProjection: Entity {
    fn frontend_excluded_fields() -> Vec<&'static str> { Vec::new() }
    fn filter_for_frontend(&self) -> Value;   // removes excluded fields
}
```

`Repository::find_for_frontend` / `find_by_id_for_frontend` require
`E: FrontendProjection`.

### Timestamps

```rust
pub trait Timestamps {
    fn created_at(&self) -> Option<DateTime<Utc>>;
    fn updated_at(&self) -> Option<DateTime<Utc>>;
    fn set_created_at(&mut self, ts: DateTime<Utc>);
    fn set_updated_at(&mut self, ts: DateTime<Utc>);
    fn apply_timestamps_for_insert(&mut self);
    fn apply_timestamps_for_update(&mut self);
}
```

`apply_timestamps(&mut doc, is_insert)` is the free function used by
`Repository`. `TimestampFields` and `timestamp_now_rfc3339()` are helpers.

### Validate

```rust
pub trait Validate {
    fn validate(&self) -> OrmResult<()>;
}
```

Blanket impls exist for `Option<T>` and `Vec<T>` where `T: Validate`.

---

## Derive macros (`nosql_orm_derive`)

Available derives: `Model`, `OrmEntity`, `Entity`, `ModelHelper`, `Validate`.

`Model` and `OrmEntity` both generate `Entity` + `WithRelations` (empty).
`OrmEntity` additionally derives `Serialize, Deserialize, Debug`. `Entity`
generates only the trait impls (you keep your own derives).

### Container attributes

| Attribute | Form | Effect |
|---|---|---|
| `table_name` / `entity` | `#[table_name("users")]` or `#[table_name("users")]` | collection/table name |
| `id_field` | `#[id_field("user_id")]` | primary key field (default `id`) |
| `soft_delete` | flag | sets `is_soft_deletable() = true` and implements `SoftDeletable` (requires a `deleted_at: Option<DateTime<Utc>>` field) |
| `timestamp` | flag | implements `Timestamps` (requires `created_at`/`updated_at` fields) |
| `frontend_exclude` | `#[frontend_exclude("secret")]` or list | implements `FrontendProjection` |
| `index` | `#[index("email,1,unique")]` or `#[index("email", 1)]`-style list | adds a `NosqlIndex` |
| `sql_column` | `#[sql_column("id,serial,primary")]` | adds a `SqlColumnDef` |
| `one_to_many` / `many_to_one` / `one_to_one` / `many_to_many` | `#[many_to_one("author", "users", "author_id")]` | adds a `RelationDef` |
| `Relations` / `relations` | list of names | generates `one_to_many` relations using snake-cased collection and `<name>_id` key |
| `counter` | (accepted) | parsed but currently no effect |

### Validation attributes (`#[validate(...)]`, via `Validate` derive)

| Attribute | Example |
|---|---|
| `email` | `#[validate(email)]` |
| `uuid` | `#[validate(uuid)]` |
| `url` | `#[validate(url)]` |
| `not_empty` | `#[validate(not_empty)]` |
| `non_null` | `#[validate(non_null)]` |
| `required` | `#[validate(required)]` |
| `length(min = N, max = M)` | `#[validate(length(min = 2, max = 50))]` |
| `min = N` / `max = N` | `#[validate(min = 18)]` |
| `range(min = N, max = M)` | `#[validate(range(min = 0, max = 100))]` |
| `pattern("regex")` | `#[validate(pattern(r"^[a-z]+$"))]` |

`#[derive(Validate)]` also generates validators for `Option<T>` and `Vec<T>`
fields automatically.

---

## Field metadata

```rust
pub enum FieldType { Id, Column, Relation(RelationFieldType) }

pub enum RelationFieldType { OneToOne, OneToMany, ManyToOne, ManyToMany, ManyToOneArray }

pub struct FieldMeta {
    pub name: String,
    pub field_type: FieldType,
    pub relation: Option<RelationMeta>,
    pub validators: Vec<ValidateMeta>,
    pub is_optional: bool,
    pub is_timestamp: bool,
    pub is_soft_delete: bool,
}

pub struct RelationMeta { /* name, target_collection, relation_type, local/foreign keys */ }
```

`Entity::fields()` defaults to an empty `Vec`; the derive macros do not yet
populate it (manual impls may).

---

## SQL columns

```rust
pub struct SqlColumnDef {
    pub name: String,
    pub column_type: SqlColumnType,
    pub primary_key: bool,
    pub unique: bool,
    pub nullable: bool,
    pub default: Option<String>,
    pub check: Option<String>,
    pub references: Option<(String, String)>,
}

impl SqlColumnDef {
    pub fn new(name: impl Into<String>, column_type: SqlColumnType) -> Self;
    pub fn primary_key(self) -> Self;
    pub fn unique(self) -> Self;
    pub fn nullable(self) -> Self;
    pub fn default(self, value: impl Into<String>) -> Self;
    pub fn check(self, condition: impl Into<String>) -> Self;
    pub fn references(self, table: impl Into<String>, column: impl Into<String>) -> Self;
}
```

`SqlColumnType` variants: `Serial`, `BigSerial`, `Boolean`, `Integer`,
`BigInteger`, `SmallInteger`, `Float`, `Double`, `Char(usize)`,
`VarChar(usize)`, `Text`, `Date`, `Time`, `DateTime`, `Timestamp`, `Json`,
`JsonB`, `Uuid`, `Array(Box<SqlColumnType>)`.

---

## Indexes

```rust
impl Entity {
    fn indexes() -> Vec<NosqlIndex> { Vec::new() }
}
```

`NosqlIndex` constructors: `single`, `compound`, `text`, `geospatial_2dsphere`,
`geospatial_2d`, `hashed`, `ttl`. Builder methods: `name`, `unique`, `sparse`,
`partial_filter`, `default_language`, `sphere_version`, `two_d_precision`.
See [07-features/07f-indexes.md](07-features/07f-indexes.md).

---

## Examples

### Soft-deletable entity

```rust
use chrono::{DateTime, Utc};
use nosql_orm::prelude::*;
use nosql_orm::SoftDeletable;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Post {
    pub id: Option<String>,
    pub title: String,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Entity for Post {
    fn meta() -> EntityMeta { EntityMeta::new("posts") }
    fn get_id(&self) -> Option<String> { self.id.clone() }
    fn set_id(&mut self, id: String) { self.id = Some(id); }
    fn is_soft_deletable() -> bool { true }
}

impl SoftDeletable for Post {
    fn deleted_at(&self) -> Option<DateTime<Utc>> { self.deleted_at }
    fn set_deleted_at(&mut self, d: Option<DateTime<Utc>>) { self.deleted_at = d; }
}
```

### Using the macro (simplified)

```rust
use nosql_orm_derive::Model;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Model)]
#[table_name("users")]
#[soft_delete]
#[timestamp]
#[index("email,1,unique")]
#[sql_column("id,serial,primary")]
#[sql_column("name,varchar,255")]
pub struct User {
    pub id: Option<String>,
    pub name: String,
    pub email: String,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}
```

---

## Next steps

- [03-provider.md](03-provider.md) — Providers
- [07-features/07a-macros.md](07-features/07a-macros.md) — Macros
- [07-features/07b-validators.md](07-features/07b-validators.md) — Validators
- [07-features/07c-soft-delete.md](07-features/07c-soft-delete.md) — Soft delete