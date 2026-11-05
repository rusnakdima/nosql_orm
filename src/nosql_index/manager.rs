use crate::error::{OrmError, OrmResult};
use crate::nosql_index::definition::NosqlIndex;
use crate::provider::{Capability, Storage};

#[derive(Clone)]
pub struct IndexManager<P: Storage> {
  provider: P,
}

impl<P: Storage> IndexManager<P> {
  pub fn new(provider: P) -> Self {
    Self { provider }
  }

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

  pub async fn create_2dsphere_index(&self, collection: &str, field: &str) -> OrmResult<()> {
    let index = NosqlIndex::geospatial_2dsphere(field);
    self.provider.create_index(collection, &index).await
  }

  pub async fn create_2d_index(&self, collection: &str, field: &str) -> OrmResult<()> {
    let index = NosqlIndex::geospatial_2d(field);
    self.provider.create_index(collection, &index).await
  }

  pub async fn create_ttl_index(
    &self,
    collection: &str,
    field: &str,
    expire_after_seconds: u32,
  ) -> OrmResult<()> {
    let index = NosqlIndex::ttl(field, expire_after_seconds);
    self.provider.create_index(collection, &index).await
  }

  pub async fn create_hashed_index(&self, collection: &str, field: &str) -> OrmResult<()> {
    let index = NosqlIndex::hashed(field);
    self.provider.create_index(collection, &index).await
  }

  pub async fn drop_index(&self, collection: &str, index_name: &str) -> OrmResult<()> {
    self.provider.drop_index(collection, index_name).await
  }

  pub async fn list_indexes(
    &self,
    collection: &str,
  ) -> OrmResult<Vec<super::definition::NosqlIndexInfo>> {
    let indexes = self.provider.list_indexes(collection).await?;
    Ok(
      indexes
        .into_iter()
        .map(|idx| super::definition::NosqlIndexInfo {
          name: idx.name,
          namespace: idx.collection,
          unique: idx.unique,
          sparse: idx.sparse,
          ttl_seconds: None,
          version: None,
          index_type: idx.index_type,
          fields: idx.fields.into_iter().map(|f| (f, 1i32)).collect(),
        })
        .collect(),
    )
  }

  pub async fn index_exists(&self, collection: &str, index_name: &str) -> OrmResult<bool> {
    self.provider.index_exists(collection, index_name).await
  }

  pub async fn ensure_index(&self, collection: &str, index: NosqlIndex) -> OrmResult<bool> {
    let name = index.get_name().unwrap_or("unnamed");
    if self.index_exists(collection, name).await? {
      return Ok(false);
    }
    self.provider.create_index(collection, &index).await?;
    Ok(true)
  }

  pub async fn sync_from_entity<E: crate::entity::Entity>(
    &self,
    collection: &str,
  ) -> OrmResult<Vec<String>> {
    if !self.provider.supports(Capability::Indexes) {
      return Err(OrmError::NotSupported(format!(
        "provider '{}' does not support indexes",
        self.provider_name()
      )));
    }
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

  pub async fn drop_all_indexes(&self, collection: &str) -> OrmResult<()> {
    if !self.provider.supports(Capability::Indexes) {
      return Err(OrmError::NotSupported(format!(
        "provider '{}' does not support indexes",
        self.provider_name()
      )));
    }
    let indexes = self.provider.list_indexes(collection).await?;
    for idx in indexes {
      // Don't drop _id index
      if idx.name != "_id_" {
        self.provider.drop_index(collection, &idx.name).await?;
      }
    }
    Ok(())
  }

  fn provider_name(&self) -> &'static str {
    std::any::type_name::<P>()
  }
}
