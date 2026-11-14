use crate::entity::Entity;
use crate::error::OrmResult;
use crate::inheritance::{Discriminator, Inheritance};
use crate::provider::Storage;
use crate::query::Filter;
use crate::repository::{Repository, RepositoryQuery};
use serde_json::{json, Value};
use std::collections::HashMap;

impl<E, P> Repository<E, P>
where
  E: Entity + Inheritance,
  P: Storage,
{
  pub fn query_scoped(&self) -> RepositoryQuery<'_, E, P> {
    let query = self.query();
    match E::discriminator_value() {
      Some(value) => query.filter(Filter::Eq(
        E::discriminator_column().to_string(),
        json!(value.as_str()),
      )),
      None => query,
    }
  }

  pub async fn find_all_scoped(&self) -> OrmResult<Vec<E>> {
    self.query_scoped().find().await
  }

  pub async fn find_by_id_scoped(&self, id: impl AsRef<str>) -> OrmResult<Option<E>> {
    let doc = match self
      .provider
      .find_by_id(&E::table_name(), id.as_ref())
      .await?
    {
      Some(doc) => doc,
      None => return Ok(None),
    };
    if let Some(value) = E::discriminator_value() {
      let matches = doc
        .get(E::discriminator_column())
        .and_then(|v| v.as_str())
        .is_some_and(|v| v == value.as_str());
      if !matches {
        return Ok(None);
      }
    }
    Ok(Some(E::from_value(doc)?))
  }

  pub fn query_in_parent(&self, parent_table: &str) -> RepositoryQuery<'_, E, P> {
    self.query_scoped().with_table(parent_table.to_string())
  }

  pub async fn find_all_in_parent(&self, parent_table: &str) -> OrmResult<Vec<E>> {
    self.query_in_parent(parent_table).find().await
  }

  pub async fn find_all_grouped_in_parent(
    &self,
    parent_table: &str,
    discriminator: &Discriminator,
  ) -> OrmResult<HashMap<String, Vec<Value>>> {
    let docs = self.provider.find_all(parent_table).await?;
    let mut groups: HashMap<String, Vec<Value>> = HashMap::new();
    for doc in docs {
      let key = match doc.get(&discriminator.column).and_then(|v| v.as_str()) {
        Some(value) => discriminator
          .get_entity_type(value)
          .map(String::from)
          .unwrap_or_else(|| value.to_string()),
        None => "unknown".to_string(),
      };
      groups.entry(key).or_default().push(doc);
    }
    Ok(groups)
  }
}
