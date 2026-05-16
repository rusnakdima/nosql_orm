use super::types::{RelationDef, WithRelations};
use crate::entity::Entity;
use crate::error::OrmResult;
use std::sync::RwLock;

static RELATION_REGISTRY: RwLock<Option<std::collections::HashMap<String, Vec<RelationDef>>>> =
  RwLock::new(None);

static REGISTERED_COLLECTIONS: RwLock<Option<std::collections::HashMap<String, bool>>> =
  RwLock::new(None);

pub fn register_collection_relations(collection: &str, relations: Vec<RelationDef>) {
  let mut registered = match REGISTERED_COLLECTIONS.write() {
    Ok(g) => g,
    Err(_) => {
      return;
    }
  };
  if registered
    .as_ref()
    .is_some_and(|r| r.contains_key(collection))
  {
    return;
  }

  let mut guard = match RELATION_REGISTRY.write() {
    Ok(g) => g,
    Err(_) => {
      return;
    }
  };
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

pub fn get_collection_relations(collection: &str) -> Option<Vec<RelationDef>> {
  let guard = match RELATION_REGISTRY.read() {
    Ok(g) => g,
    Err(_) => return None,
  };
  guard
    .as_ref()
    .and_then(|registry| registry.get(collection).cloned())
}

#[doc(hidden)]
pub fn get_registered_collection_relations(collection: &str) -> Option<Vec<RelationDef>> {
  get_collection_relations(collection)
}

pub fn register_relations_for_entity<E: WithRelations + Entity>() {
  let collection = E::table_name();
  let relations = E::relations();
  if !relations.is_empty() {
    register_collection_relations(&collection, relations);
  }
}

pub fn get_relation_def(collection: &str, relation_name: &str) -> Option<RelationDef> {
  let guard = match RELATION_REGISTRY.read() {
    Ok(g) => g,
    Err(_) => return None,
  };
  guard.as_ref().and_then(|registry| {
    registry
      .get(collection)
      .and_then(|relations| relations.iter().find(|r| r.name == relation_name).cloned())
  })
}

pub fn clear_relation_registry() {
  if let Ok(mut guard) = RELATION_REGISTRY.write() {
    *guard = None;
  }
}

pub fn clear_registered_collections() {
  if let Ok(mut guard) = REGISTERED_COLLECTIONS.write() {
    *guard = None;
  }
}

pub fn clear_all_registries() -> OrmResult<()> {
  clear_relation_registry();
  clear_registered_collections();
  Ok(())
}

pub fn is_relation_registry_empty() -> bool {
  let guard = match RELATION_REGISTRY.read() {
    Ok(g) => g,
    Err(_) => return true,
  };
  guard.as_ref().map_or(true, |r| r.is_empty())
}

pub fn is_registered_collections_empty() -> bool {
  let guard = match REGISTERED_COLLECTIONS.read() {
    Ok(g) => g,
    Err(_) => return true,
  };
  guard.as_ref().map_or(true, |r| r.is_empty())
}

pub fn is_all_empty() -> bool {
  is_relation_registry_empty() && is_registered_collections_empty()
}
