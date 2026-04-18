# NoSQL Indexes Implementation Plan

## Current State

The library currently has a **SQL-oriented `IndexDef`** in `src/constraints/index.rs`:
- Uses SQL terms: `BTree`, `Hash`, `GiST`, `GIN`
- Has `to_sql()` method for SQL generation
- `SchemaManager` has placeholder methods `create_collection()` and `drop_collection()` that don't actually create indexes

**This is NOT suitable for NoSQL databases.**

---

## What Are NoSQL Indexes?

### MongoDB Index Types

| Index Type | Description | Use Case |
|------------|-------------|----------|
| **Single Field** | `{ field: 1 }` | Basic queries on one field |
| **Compound** | `{ field1: 1, field2: -1 }` | Multi-field queries |
| **Multikey** | Auto on array fields | Query arrays |
| **Text** | Full-text search | Search in string content |
| **2dsphere** | Geospatial on sphere | Location queries |
| **2d** | Geospatial on flat | Legacy geo queries |
| **Hashed** | Hash of field value | Sharding |

### Index Options

| Option | Description |
|--------|-------------|
| **unique** | No duplicate values |
| **sparse** | Only index non-null values |
| **TTL** | Auto-delete after N seconds |
| **partialFilterExpression** | Conditional indexing |

---

## Implementation Plan

### Phase 1: Create NoSQL Index Abstraction

**New file: `src/nosql_index/mod.rs`**

```rust
// Index types specific to NoSQL/MongoDB
pub enum NosqlIndexType {
    SingleField,      // Default index
    Compound,        // Multiple fields
    Text,           // Full-text search
    Geospatial2dsphere,  // Spherical geo queries
    Geospatial2d,    // Flat geo queries
    Hashed,         // Hash for sharding
    TTL,            // Auto-expiration
}

pub struct NosqlIndex {
    pub name: Option<String>,
    pub fields: Vec<(String, i32)>,  // field name + sort order (1 or -1)
    pub index_type: NosqlIndexType,
    pub unique: bool,
    pub sparse: bool,
    pub ttl_seconds: Option<u32>,           // For TTL indexes
    pub partial_filter: Option<Filter>,     // For partial indexes
    pub weights: Option<HashMap<String, i32>>, // For text indexes
    pub default_language: Option<String>,   // For text indexes
}
```

### Phase 2: Extend DatabaseProvider with Index Management

**Add to `src/provider.rs`:**

```rust
#[async_trait]
pub trait DatabaseProvider: Send + Sync + Clone + 'static {
    // ... existing methods ...

    // ── Index Management ──────────────────────────────────────

    /// Create an index on a collection.
    async fn create_index(
        &self,
        collection: &str,
        index: &NosqlIndex,
    ) -> OrmResult<()>;

    /// Drop an index by name.
    async fn drop_index(
        &self,
        collection: &str,
        index_name: &str,
    ) -> OrmResult<()>;

    /// List all indexes on a collection.
    async fn list_indexes(
        &self,
        collection: &str,
    ) -> OrmResult<Vec<NosqlIndexInfo>>;

    /// Check if an index exists.
    async fn index_exists(
        &self,
        collection: &str,
        index_name: &str,
    ) -> OrmResult<bool>;
}
```

### Phase 3: Implement MongoDB Index Provider

**Update `src/providers/mongo.rs`:**

```rust
impl MongoProvider {
    /// Create a MongoDB index.
    pub async fn create_mongo_index(
        &self,
        collection: &str,
        index: &NosqlIndex,
    ) -> OrmResult<String> {
        // Build MongoDB index keys document
        let keys = doc! {
            for (field, order) in &index.fields {
                field: order
            }
        };

        // Build index options
        let mut opts = IndexOptions::default();

        if let Some(name) = &index.name {
            opts.name = Some(name.clone());
        }
        opts.unique = Some(index.unique);
        opts.sparse = Some(index.sparse);
        opts.partial_filter_expression = index.partial_filter
            .as_ref()
            .map(|f| Self::filter_to_doc(f));

        if let Some(ttl) = index.ttl_seconds {
            opts.expire_after = Some(Duration::seconds(ttl as i64));
        }

        if let Some(ref weights) = index.weights {
            let mut doc = doc! {};
            for (field, weight) in weights {
                doc.insert(field, weight);
            }
            opts.weights = Some(doc);
        }

        let model = IndexModel::builder()
            .keys(keys)
            .options(opts)
            .build();

        let coll = self.db.collection::<Document>(collection);
        coll.create_index(model, None).await?;

        Ok(index.name.clone().unwrap_or_default())
    }

    /// Get all indexes on a collection.
    pub async fn list_mongo_indexes(
        &self,
        collection: &str,
    ) -> OrmResult<Vec<IndexInfo>> {
        let coll = self.db.collection::<Document>(collection);
        coll.list_index_names(None).await.map_err(Into::into)
    }
}
```

### Phase 4: Create IndexManager

**New file: `src/nosql_index/manager.rs`**

```rust
/// Index manager for managing collection indexes
pub struct IndexManager<P: DatabaseProvider> {
    provider: P,
}

impl<P: DatabaseProvider> IndexManager<P> {
    pub fn new(provider: P) -> Self {
        Self { provider }
    }

    /// Create a single field index.
    pub async fn create_single_field_index(
        &self,
        collection: &str,
        field: &str,
        unique: bool,
    ) -> OrmResult<()> {
        let index = NosqlIndex::single(field, 1).unique(unique);
        self.provider.create_index(collection, &index).await
    }

    /// Create a compound index.
    pub async fn create_compound_index(
        &self,
        collection: &str,
        fields: &[(&str, i32)],  // field + sort order
        unique: bool,
    ) -> OrmResult<()> {
        let index = NosqlIndex::compound(fields).unique(unique);
        self.provider.create_index(collection, &index).await
    }

    /// Create a text index for full-text search.
    pub async fn create_text_index(
        &self,
        collection: &str,
        fields: &[(&str, i32)],  // field + weight
        default_language: Option<&str>,
    ) -> OrmResult<()> {
        let index = NosqlIndex::text(fields).default_language(default_language);
        self.provider.create_index(collection, &index).await
    }

    /// Create a TTL index for auto-expiration.
    pub async fn create_ttl_index(
        &self,
        collection: &str,
        field: &str,
        expire_after_seconds: u32,
    ) -> OrmResult<()> {
        let index = NosqlIndex::ttl(field, expire_after_seconds);
        self.provider.create_index(collection, &index).await
    }

    /// Create a geospatial 2dsphere index.
    pub async fn create_2dsphere_index(
        &self,
        collection: &str,
        field: &str,
    ) -> OrmResult<()> {
        let index = NosqlIndex::geospatial_2dsphere(field);
        self.provider.create_index(collection, &index).await
    }

    /// Drop index by name.
    pub async fn drop_index(
        &self,
        collection: &str,
        index_name: &str,
    ) -> OrmResult<()> {
        self.provider.drop_index(collection, index_name).await
    }

    /// Sync indexes from entity definition.
    pub async fn sync_from_entity<E: Entity>(
        &self,
        collection: &str,
    ) -> OrmResult<Vec<String>> {
        let mut created = Vec::new();
        for index_def in E::indexes() {
            self.provider.create_index(collection, &index_def).await?;
            created.push(index_def.name.unwrap_or_default());
        }
        Ok(created)
    }
}
```

### Phase 5: Add Entity Index Definitions

**Update `src/entity.rs`:**

```rust
pub trait Entity: Send + Sync {
    // ... existing methods ...

    /// Returns indexes defined for this entity.
    fn indexes() -> Vec<NosqlIndex> {
        Vec::new()  // Default: no indexes
    }
}

/// Macro to define indexes on entity
#[macro_export]
macro_rules! entity_indexes {
    ($entity:ident, $($index:expr),*) => {
        fn indexes() -> Vec<NosqlIndex> {
            vec![$($index),*]
        }
    };
}
```

### Phase 6: Add Repository Index Methods

**Update `src/repository.rs`:**

```rust
impl<E, P> Repository<E, P>
where
    E: Entity,
    P: DatabaseProvider,
{
    // ... existing methods ...

    /// Get the index manager for this repository's collection.
    pub fn indexes(&self) -> IndexManager<P> {
        IndexManager::new(self.provider.clone())
    }

    /// Sync indexes from entity definition.
    pub async fn sync_indexes(&self) -> OrmResult<Vec<String>> {
        let manager = self.indexes();
        manager.sync_from_entity::<E>(&Self::collection()).await
    }

    /// Create index on this collection.
    pub async fn create_index(&self, index: NosqlIndex) -> OrmResult<()> {
        self.provider
            .create_index(&Self::collection(), &index)
            .await
    }

    /// Drop index by name.
    pub async fn drop_index(&self, name: &str) -> OrmResult<()> {
        self.provider.drop_index(&Self::collection(), name).await
    }
}
```

---

## Usage Examples

### Basic Usage

```rust
use nosql_orm::prelude::*;

// Create single field index
repo.create_index(
    NosqlIndex::single("email", 1).unique(true).name("idx_email")
).await?;

// Create compound index
repo.create_index(
    NosqlIndex::compound(&[
        ("user_id", 1),
        ("created_at", -1),
    ]).name("idx_user_date")
).await?;

// Create TTL index (auto-delete after 30 days)
repo.create_index(
    NosqlIndex::ttl("created_at", 30 * 24 * 60 * 60).name("idx_ttl")
).await?;

// Create text index for search
repo.create_index(
    NosqlIndex::text(&[("title", 10), ("description", 5)])
        .default_language("en")
        .name("idx_text")
).await?;

// Using IndexManager
repo.indexes()
    .create_compound_index(&[("status", 1), ("priority", -1)], false)
    .await?;

// Sync indexes from entity definition
repo.sync_indexes().await?;
```

### Entity with Indexes

```rust
use nosql_orm::prelude::*;

#[derive(Entity)]
pub struct User {
    pub id: Option<String>,
    pub email: String,
    pub created_at: DateTime<Utc>,
    pub status: String,
}

impl Entity for User {
    fn meta() -> EntityMeta { EntityMeta::new("users") }
    fn get_id(&self) -> Option<String> { self.id.clone() }
    fn set_id(&mut self, id: String) { self.id = Some(id); }

    fn indexes() -> Vec<NosqlIndex> {
        vec![
            NosqlIndex::single("email", 1).unique(true).name("idx_email"),
            NosqlIndex::ttl("created_at", 30 * 24 * 60 * 60).name("idx_ttl"),
            NosqlIndex::compound(&[("status", 1), ("created_at", -1)]).name("idx_status_date"),
        ]
    }
}
```

---

## Files to Create/Modify

| File | Action |
|------|--------|
| `src/nosql_index/mod.rs` | Create - NoSQL index types |
| `src/nosql_index/manager.rs` | Create - IndexManager |
| `src/nosql_index/macros.rs` | Create - Helper macros |
| `src/provider.rs` | Modify - Add index management trait methods |
| `src/providers/mongo.rs` | Modify - Implement MongoDB index operations |
| `src/providers/json.rs` | Modify - Implement (no-op or basic) |
| `src/providers/redis.rs` | Modify - Implement (no-op or basic) |
| `src/repository.rs` | Modify - Add index methods |
| `src/entity.rs` | Modify - Add indexes() method |
| `src/lib.rs` | Modify - Export new types |
| `examples/index_example.rs` | Create - Usage examples |

---

## Priority Order

1. **Phase 1-2**: Create abstraction + extend provider trait
2. **Phase 3**: Implement MongoDB indexes (most important for NoSQL)
3. **Phase 4**: Create IndexManager
4. **Phase 5**: Add Entity indexes support
5. **Phase 6**: Repository integration + JSON/Redis placeholders
