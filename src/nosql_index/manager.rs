//! Index manager for creating and managing NoSQL indexes.

use crate::error::OrmResult;
use crate::nosql_index::NosqlIndex;
use crate::provider::DatabaseProvider;

/// Index manager for managing collection indexes.
///
/// Provides a convenient API for creating and managing indexes
/// on collections backed by a DatabaseProvider.
///
/// # Example
///
/// ```rust,ignore
/// use nosql_orm::nosql_index::IndexManager;
///
/// let manager = IndexManager::new(provider);
///
/// // Create indexes
/// manager.create_single_field_index("users", "email", true).await?;
/// manager.create_compound_index("users", &[("status", 1), ("created_at", -1)], false).await?;
/// manager.create_text_index("posts", &[("title", 10), ("body", 5)], Some("en")).await?;
///
/// // Sync indexes from entity
/// manager.sync_from_entity::<User>("users").await?;
/// ```
#[derive(Clone)]
pub struct IndexManager<P: DatabaseProvider> {
  provider: P,
}

impl<P: DatabaseProvider> IndexManager<P> {
  /// Create a new index manager with the given provider.
  pub fn new(provider: P) -> Self {
    Self { provider }
  }

  /// Create a single field index.
  ///
  /// # Arguments
  /// * `collection` - Collection name.
  /// * `field` - Field to index.
  /// * `order` - Sort order (1 = ascending, -1 = descending).
  /// * `unique` - Whether the index should be unique.
  ///
  /// # Example
  ///
  /// ```rust,ignore
  /// manager.create_single_field_index("users", "email", 1, true).await?;
  /// ```
  pub async fn create_single_field_index(
    &self,
    collection: &str,
    field: &str,
    order: i32,
    unique: bool,
  ) -> OrmResult<()> {
    let index = if unique {
      NosqlIndex::single(field, order).unique()
    } else {
      NosqlIndex::single(field, order)
    };
    self.provider.create_index(collection, &index).await
  }

  /// Create a compound index on multiple fields.
  ///
  /// # Arguments
  /// * `collection` - Collection name.
  /// * `fields` - Slice of (field, order) tuples.
  /// * `unique` - Whether the index should be unique.
  ///
  /// # Example
  ///
  /// ```rust,ignore
  /// manager.create_compound_index(
  ///     "users",
  ///     &[("status", 1), ("created_at", -1)],
  ///     false
  /// ).await?;
  /// ```
  pub async fn create_compound_index(
    &self,
    collection: &str,
    fields: &[(&str, i32)],
    unique: bool,
  ) -> OrmResult<()> {
    let index = if unique {
      NosqlIndex::compound(fields).unique()
    } else {
      NosqlIndex::compound(fields)
    };
    self.provider.create_index(collection, &index).await
  }

  /// Create a text index for full-text search.
  ///
  /// # Arguments
  /// * `collection` - Collection name.
  /// * `fields` - Slice of (field, weight) tuples. Higher weights
  ///   mean higher relevance scores.
  /// * `default_language` - Default language for text search (e.g., "en").
  ///
  /// # Example
  ///
  /// ```rust,ignore
  /// manager.create_text_index(
  ///     "posts",
  ///     &[("title", 10), ("body", 5)],
  ///     Some("en")
  /// ).await?;
  /// ```
  pub async fn create_text_index(
    &self,
    collection: &str,
    fields: &[(&str, i32)],
    default_language: Option<&str>,
  ) -> OrmResult<()> {
    let mut index = NosqlIndex::text(fields);
    if let Some(lang) = default_language {
      index = index.default_language(lang);
    }
    self.provider.create_index(collection, &index).await
  }

  /// Create a geospatial 2dsphere index for GeoJSON points.
  ///
  /// # Example
  ///
  /// ```rust,ignore
  /// manager.create_2dsphere_index("places", "location").await?;
  /// ```
  pub async fn create_2dsphere_index(&self, collection: &str, field: &str) -> OrmResult<()> {
    let index = NosqlIndex::geospatial_2dsphere(field);
    self.provider.create_index(collection, &index).await
  }

  /// Create a legacy 2d geospatial index (flat plane).
  ///
  /// # Example
  ///
  /// ```rust,ignore
  /// manager.create_2d_index("points", "coords").await?;
  /// ```
  pub async fn create_2d_index(&self, collection: &str, field: &str) -> OrmResult<()> {
    let index = NosqlIndex::geospatial_2d(field);
    self.provider.create_index(collection, &index).await
  }

  /// Create a TTL index for automatic document expiration.
  ///
  /// Documents will be automatically deleted after `expire_after_seconds`
  /// from the value in the date field.
  ///
  /// # Arguments
  /// * `collection` - Collection name.
  /// * `field` - Date field to use for TTL calculation.
  /// * `expire_after_seconds` - Seconds after the field date to delete.
  ///
  /// # Example
  ///
  /// ```rust,ignore
  /// // Delete documents 30 days after created_at
  /// manager.create_ttl_index("sessions", "created_at", 30 * 24 * 60 * 60).await?;
  /// ```
  pub async fn create_ttl_index(
    &self,
    collection: &str,
    field: &str,
    expire_after_seconds: u32,
  ) -> OrmResult<()> {
    let index = NosqlIndex::ttl(field, expire_after_seconds);
    self.provider.create_index(collection, &index).await
  }

  /// Create a hashed index for sharding.
  ///
  /// # Example
  ///
  /// ```rust,ignore
  /// manager.create_hashed_index("users", "user_id").await?;
  /// ```
  pub async fn create_hashed_index(&self, collection: &str, field: &str) -> OrmResult<()> {
    let index = NosqlIndex::hashed(field);
    self.provider.create_index(collection, &index).await
  }

  /// Drop an index by name.
  ///
  /// # Example
  ///
  /// ```rust,ignore
  /// manager.drop_index("users", "idx_email").await?;
  /// ```
  pub async fn drop_index(&self, collection: &str, index_name: &str) -> OrmResult<()> {
    self.provider.drop_index(collection, index_name).await
  }

  /// List all indexes on a collection.
  ///
  /// # Example
  ///
  /// ```rust,ignore
  /// let indexes = manager.list_indexes("users").await?;
  /// for idx in indexes {
  ///     println!("Index: {}", idx.name);
  /// }
  /// ```
  pub async fn list_indexes(
    &self,
    collection: &str,
  ) -> OrmResult<Vec<crate::nosql_index::NosqlIndexInfo>> {
    self.provider.list_indexes(collection).await
  }

  /// Check if an index exists by name.
  ///
  /// # Example
  ///
  /// ```rust,ignore
  /// if manager.index_exists("users", "idx_email").await? {
  ///     println!("Index exists!");
  /// }
  /// ```
  pub async fn index_exists(&self, collection: &str, index_name: &str) -> OrmResult<bool> {
    self.provider.index_exists(collection, index_name).await
  }

  /// Ensure an index exists, creating it if it doesn't.
  ///
  /// This is useful for migration scripts that need to ensure
  /// certain indexes are present.
  ///
  /// # Example
  ///
  /// ```rust,ignore
  /// manager.ensure_index(
  ///     "users",
  ///     NosqlIndex::single("email", 1).unique(true).name("idx_email")
  /// ).await?;
  /// ```
  pub async fn ensure_index(&self, collection: &str, index: NosqlIndex) -> OrmResult<bool> {
    let name = index.get_name().unwrap_or("unnamed");
    if self.index_exists(collection, name).await? {
      return Ok(false);
    }
    self.provider.create_index(collection, &index).await?;
    Ok(true)
  }

  /// Sync indexes from an entity definition.
  ///
  /// Creates any indexes defined on the entity that don't exist yet.
  ///
  /// # Example
  ///
  /// ```rust,ignore
  /// let created = manager.sync_from_entity::<User>("users").await?;
  /// println!("Created {} indexes", created.len());
  /// ```
  pub async fn sync_from_entity<E: crate::entity::Entity>(
    &self,
    collection: &str,
  ) -> OrmResult<Vec<String>> {
    let mut created = Vec::new();
    for index in E::indexes() {
      let name = index.get_name().unwrap_or("unnamed").to_string();
      if !self.index_exists(collection, &name).await? {
        self.provider.create_index(collection, &index).await?;
        created.push(name);
      }
    }
    Ok(created)
  }

  /// Drop all indexes on a collection except the default _id index.
  ///
  /// # Example
  ///
  /// ```rust,ignore
  /// manager.drop_all_indexes("users").await?;
  /// ```
  pub async fn drop_all_indexes(&self, collection: &str) -> OrmResult<()> {
    let indexes = self.provider.list_indexes(collection).await?;
    for idx in indexes {
      // Don't drop _id index
      if idx.name != "_id_" {
        self.provider.drop_index(collection, &idx.name).await?;
      }
    }
    Ok(())
  }
}
