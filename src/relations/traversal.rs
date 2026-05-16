use crate::entity::Entity;
use crate::error::OrmResult;
use crate::provider::DatabaseProvider;
use crate::relations::{get_relation_def, WithRelations};

pub struct RelationTraversal;

impl RelationTraversal {
  pub async fn resolve_via<E, P>(
    provider: &P,
    start_id: &str,
    relation_path: &[(&str, &str)],
  ) -> OrmResult<Option<String>>
  where
    E: Entity + WithRelations,
    P: DatabaseProvider,
  {
    let mut current_id = start_id.to_string();
    let mut current_table = E::table_name().to_string();

    for (target_table, foreign_key) in relation_path {
      let filter = crate::query::Filter::Eq("id".to_string(), serde_json::json!(current_id));

      let docs = provider
        .find_many(&current_table, Some(&filter), None, Some(1), None, true)
        .await?;

      let doc = match docs.into_iter().next() {
        Some(d) => d,
        None => return Ok(None),
      };

      current_id = match doc.get(*foreign_key).and_then(|v| v.as_str()) {
        Some(id) => id.to_string(),
        None => return Ok(None),
      };

      current_table = (*target_table).to_string();
    }

    Ok(Some(current_id))
  }

  pub async fn resolve_to_root<E, P>(
    provider: &P,
    table: &str,
    id: &str,
    root_relation: &str,
  ) -> OrmResult<Option<String>>
  where
    E: Entity + WithRelations,
    P: DatabaseProvider,
  {
    let rel = get_relation_def(table, root_relation).ok_or_else(|| {
      crate::error::OrmError::InvalidQuery(format!("Relation '{}' not found", root_relation))
    })?;

    let filter = crate::query::Filter::Eq("id".to_string(), serde_json::json!(id));

    let docs = provider
      .find_many(table, Some(&filter), None, Some(1), None, true)
      .await?;

    let doc = match docs.into_iter().next() {
      Some(d) => d,
      None => return Ok(None),
    };

    match doc.get(&rel.foreign_key).and_then(|v| v.as_str()) {
      Some(root_id) => Ok(Some(root_id.to_string())),
      None => Ok(None),
    }
  }

  pub async fn get_parent_id<P>(
    provider: &P,
    table: &str,
    id: &str,
    parent_field: &str,
  ) -> OrmResult<Option<String>>
  where
    P: DatabaseProvider,
  {
    let filter = crate::query::Filter::Eq("id".to_string(), serde_json::json!(id));

    let docs = provider
      .find_many(table, Some(&filter), None, Some(1), None, true)
      .await?;

    match docs.into_iter().next() {
      Some(doc) => Ok(
        doc
          .get(parent_field)
          .and_then(|v| v.as_str())
          .map(String::from),
      ),
      None => Ok(None),
    }
  }
}
