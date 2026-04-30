use super::types::{RelationDef, WithRelations};
use crate::entity::Entity;

static RELATION_REGISTRY: std::sync::RwLock<
  Option<std::collections::HashMap<String, Vec<RelationDef>>>,
> = std::sync::RwLock::new(None);

static REGISTERED_COLLECTIONS: std::sync::RwLock<Option<std::collections::HashMap<String, bool>>> =
  std::sync::RwLock::new(None);

pub fn register_collection_relations(collection: &str, relations: Vec<RelationDef>) {
  let mut registered = REGISTERED_COLLECTIONS.write().unwrap();
  if registered
    .as_ref()
    .is_some_and(|r| r.contains_key(collection))
  {
    return;
  }

  let mut guard = RELATION_REGISTRY.write().unwrap();
  if guard.is_none() {
    *guard = Some(std::collections::HashMap::new());
  }
  if let Some(registry) = guard.as_mut() {
    registry.insert(collection.to_string(), relations);
  }
  drop(guard);

  if registered.is_none() {
    *registered = Some(std::collections::HashMap::new());
  }
  if let Some(registered) = registered.as_mut() {
    registered.insert(collection.to_string(), true);
  }
}

pub fn get_registered_collection_relations(collection: &str) -> Option<Vec<RelationDef>> {
  let guard = RELATION_REGISTRY.read().unwrap();
  guard
    .as_ref()
    .and_then(|registry| registry.get(collection).cloned())
}

pub fn get_collection_relations(collection: &str) -> Option<Vec<RelationDef>> {
  let guard = RELATION_REGISTRY.read().unwrap();
  guard
    .as_ref()
    .and_then(|registry| registry.get(collection).cloned())
}

pub fn register_relations_for_entity<E: WithRelations + Entity>() {
  let collection = E::table_name();
  let relations = E::relations();
  if !relations.is_empty() {
    register_collection_relations(&collection, relations);
  }
}

pub fn get_relation_def(collection: &str, relation_name: &str) -> Option<RelationDef> {
  let guard = RELATION_REGISTRY.read().unwrap();
  guard.as_ref().and_then(|registry| {
    registry
      .get(collection)
      .and_then(|relations| relations.iter().find(|r| r.name == relation_name).cloned())
  })
}

#[allow(dead_code)]
pub fn clear_relation_registry() {
  let mut guard = RELATION_REGISTRY.write().unwrap();
  *guard = None;
  let mut registered = REGISTERED_COLLECTIONS.write().unwrap();
  *registered = None;
}
