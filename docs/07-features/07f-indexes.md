# Indexes

NoSQL index management.

---

## NosqlIndex

```rust
pub struct NosqlIndex {
    fields: Vec<(String, i32)>,
    index_type: NosqlIndexType,
    unique: bool,
    sparse: bool,
    name: Option<String>,
    ttl_seconds: Option<u32>,
    partial_filter: Option<Filter>,
    weights: Option<HashMap<String, i32>>,
    default_language: Option<String>,
}

pub enum NosqlIndexType {
    Single,
    Compound,
    Unique,
    TTL,
    Text,
    Geospatial2d,
    Geospatial2dsphere,
    Hashed,
}
```

### Creating Indexes

```rust
// Single field
NosqlIndex::single("email", 1)
NosqlIndex::single("email", 1).unique(true)
NosqlIndex::single("email", 1).name("idx_email")

// Compound
NosqlIndex::compound(&[("field1", 1), ("field2", -1)])
NosqlIndex::compound(&[("field1", 1), ("field2", -1)]).unique(true)

// Text
NosqlIndex::text(&[("title", 10), ("body", 5)])

// Geospatial
NosqlIndex::geospatial_2d("location")
NosqlIndex::geospatial_2dsphere("location")

// Hashed
NosqlIndex::hashed("field")

// TTL (time-to-live)
NosqlIndex::ttl("created_at", 3600)
```

### Example

```rust
repo.create_index(NosqlIndex::single("email", 1)).await?;
repo.create_index(NosqlIndex::text(&[("title", 10), ("content", 5)])).await?;
repo.create_index(NosqlIndex::geospatial_2dsphere("location")).await?;
repo.create_index(NosqlIndex::hashed("author_id")).await?;
repo.create_index(NosqlIndex::ttl("created_at", 3600)).await?;

let indexes = repo.list_indexes().await?;
for idx in indexes { println!("{}", idx.name); }
repo.drop_index("idx_name").await?;
```

---

## IndexManager

```rust
let manager = IndexManager::new(provider);
manager.create_single_field_index("users", "email", 1, false).await?;
manager.list_indexes("users").await?;
```