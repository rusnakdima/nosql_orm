use nosql_orm::provider::Storage;
use nosql_orm::providers::json::JsonProvider;
use nosql_orm::relations::loader::RelationLoader;
use nosql_orm::relations::registry::{
  clear_relation_registry, get_relation_def, register_collection_relations,
};
use nosql_orm::relations::{RelationDef, RelationType, RelationValue};
use serde_json::json;

async fn create_test_loader() -> (RelationLoader<JsonProvider>, tempfile::TempDir) {
  let temp_dir = tempfile::TempDir::new().unwrap();
  let provider = JsonProvider::new(temp_dir.path()).await.unwrap();
  (RelationLoader::new(provider), temp_dir)
}

#[tokio::test]
async fn test_cascade_load_with_no_relations() {
  clear_relation_registry();

  let (loader, _tmp) = create_test_loader().await;
  let doc = json!({"name": "test"});
  let results = loader
    .load_cascade_for_entity(&doc, "users", "", true)
    .await
    .unwrap();
  assert!(results.is_empty());
}

#[tokio::test]
async fn test_cascade_load_unknown_relation() {
  clear_relation_registry();

  register_collection_relations(
    "users",
    vec![RelationDef::many_to_one(
      "profile",
      "profiles",
      "profile_id",
    )],
  );

  let (loader, _tmp) = create_test_loader().await;
  let doc = json!({"name": "test", "_collection": "users"});
  let result = loader
    .load_cascade_for_entity(&doc, "users", "nonexistent", true)
    .await;
  assert!(result.is_err());
}

#[tokio::test]
async fn test_cascade_load_empty_path() {
  clear_relation_registry();

  let (loader, _tmp) = create_test_loader().await;
  let doc = json!({"name": "test", "_collection": "users"});
  let results = loader
    .load_cascade_for_entity(&doc, "users", "", true)
    .await
    .unwrap();
  assert!(results.is_empty());
}

#[tokio::test]
async fn test_relation_def_many_to_one() {
  clear_relation_registry();

  register_collection_relations(
    "posts",
    vec![RelationDef::many_to_one("author", "users", "author_id")],
  );

  let rel = get_relation_def("posts", "author").unwrap();
  assert_eq!(rel.relation_type, RelationType::ManyToOne);
  assert_eq!(rel.name, "author");
  assert_eq!(rel.target_collection, "users");
  assert_eq!(rel.local_key, "author_id");
}

#[tokio::test]
async fn test_relation_def_one_to_many() {
  clear_relation_registry();

  register_collection_relations(
    "users",
    vec![RelationDef::one_to_many("posts", "posts", "user_id")],
  );

  let rel = get_relation_def("users", "posts").unwrap();
  assert_eq!(rel.relation_type, RelationType::OneToMany);
  assert_eq!(rel.name, "posts");
  assert_eq!(rel.target_collection, "posts");
}

#[tokio::test]
async fn test_relation_def_many_to_many() {
  clear_relation_registry();

  register_collection_relations(
    "posts",
    vec![RelationDef::many_to_many("tags", "tags", "tag_ids")],
  );

  let rel = get_relation_def("posts", "tags").unwrap();
  assert_eq!(rel.relation_type, RelationType::ManyToMany);
  assert_eq!(rel.name, "tags");
  assert_eq!(rel.target_collection, "tags");
}

#[tokio::test]
async fn test_relation_def_one_to_one() {
  clear_relation_registry();

  register_collection_relations(
    "users",
    vec![RelationDef::one_to_one("profile", "profiles", "profile_id")],
  );

  let rel = get_relation_def("users", "profile").unwrap();
  assert_eq!(rel.relation_type, RelationType::OneToOne);
  assert_eq!(rel.name, "profile");
}

#[tokio::test]
async fn test_relation_def_nonexistent() {
  clear_relation_registry();

  let rel = get_relation_def("users", "nonexistent");
  assert!(rel.is_none());
}

#[tokio::test]
async fn test_cascade_load_with_single_segment_path() {
  clear_relation_registry();

  register_collection_relations(
    "users",
    vec![RelationDef::many_to_one(
      "profile",
      "profiles",
      "profile_id",
    )],
  );

  let (loader, _tmp) = create_test_loader().await;
  let doc = json!({"name": "test", "_collection": "users"});
  let results = loader
    .load_cascade_for_entity(&doc, "users", "profile", true)
    .await;
  assert!(results.is_ok());
}

#[tokio::test]
async fn test_relation_def_builder_method_chaining() {
  clear_relation_registry();

  let rel = RelationDef::many_to_one("author", "users", "author_id");

  assert_eq!(rel.name, "author");
  assert_eq!(rel.target_collection, "users");
}

#[tokio::test]
async fn test_relation_loader_provider() {
  let (loader, _tmp) = create_test_loader().await;
  let _provider = loader.provider();
}

#[tokio::test]
async fn test_relation_loader_insert() {
  let temp_dir = tempfile::TempDir::new().unwrap();
  let provider = JsonProvider::new(temp_dir.path()).await.unwrap();
  let loader = RelationLoader::new(provider.clone());

  let doc = json!({"name": "Test User"});
  let collection = "users";

  provider.insert(collection, doc.clone()).await.unwrap();

  let found = provider
    .find_by_id(collection, "nonexistent")
    .await
    .unwrap();
  assert!(found.is_none());

  // The loader shares the same provider instance.
  let _ = loader.provider();
}

#[tokio::test]
async fn test_relation_value_single() {
  let single = RelationValue::Single(Some(json!({"name": "test"})));
  assert!(matches!(single, RelationValue::Single(Some(_))));
}

#[tokio::test]
async fn test_relation_value_many() {
  let many = RelationValue::Many(vec![json!({"name": "test1"}), json!({"name": "test2"})]);
  match many {
    RelationValue::Many(v) => assert_eq!(v.len(), 2),
    _ => panic!("expected many"),
  }
}

#[tokio::test]
async fn test_relation_registry_clear() {
  clear_relation_registry();

  register_collection_relations(
    "users",
    vec![RelationDef::many_to_one(
      "profile",
      "profiles",
      "profile_id",
    )],
  );

  clear_relation_registry();

  let rel = get_relation_def("users", "profile");
  assert!(rel.is_none());
}

fn relation_value_to_json(rv: &RelationValue) -> serde_json::Value {
  match rv {
    RelationValue::Single(opt) => {
      if let Some(v) = opt {
        json!({"single": v})
      } else {
        json!({"single": null})
      }
    }
    RelationValue::Many(arr) => json!({"many": arr}),
  }
}

#[test]
fn test_relation_value_debug() {
  let single = RelationValue::Single(Some(json!({"name": "test"})));
  let debug = format!("{:?}", single);
  assert!(debug.contains("Single"));

  let many = RelationValue::Many(vec![]);
  let debug = format!("{:?}", many);
  assert!(debug.contains("Many"));

  let _ = relation_value_to_json(&single);
  let _ = relation_value_to_json(&many);
}
