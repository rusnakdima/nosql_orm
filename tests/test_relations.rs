use nosql_orm::relations::{
  clear_relation_registry, get_collection_relations, register_collection_relations, RelationDef,
  RelationType,
};
use nosql_orm::sql::types::SqlOnDelete;

#[test]
fn test_relation_def_many_to_one() {
  let rel = RelationDef::many_to_one("user", "users", "user_id");

  assert_eq!(rel.name, "user");
  assert_eq!(rel.relation_type, RelationType::ManyToOne);
  assert_eq!(rel.target_collection, "users");
  assert_eq!(rel.local_key, "user_id");
  assert_eq!(rel.foreign_key, "id");
  assert!(rel.join_field.is_none());
}

#[test]
fn test_relation_def_one_to_many() {
  let rel = RelationDef::one_to_many("posts", "posts", "author_id");

  assert_eq!(rel.name, "posts");
  assert_eq!(rel.relation_type, RelationType::OneToMany);
  assert_eq!(rel.target_collection, "posts");
  assert_eq!(rel.local_key, "id");
  assert_eq!(rel.foreign_key, "author_id");
}

#[test]
fn test_relation_def_one_to_one() {
  let rel = RelationDef::one_to_one("profile", "profiles", "profile_id");

  assert_eq!(rel.name, "profile");
  assert_eq!(rel.relation_type, RelationType::OneToOne);
  assert_eq!(rel.target_collection, "profiles");
  assert_eq!(rel.local_key, "profile_id");
  assert_eq!(rel.foreign_key, "id");
}

#[test]
fn test_relation_def_many_to_many() {
  let rel = RelationDef::many_to_many("tags", "tags", "tag_ids");

  assert_eq!(rel.name, "tags");
  assert_eq!(rel.relation_type, RelationType::ManyToMany);
  assert_eq!(rel.target_collection, "tags");
  assert_eq!(rel.join_field, Some("tag_ids".to_string()));
}

#[test]
fn test_relation_def_many_to_many_with_read_by() {
  let rel = RelationDef::many_to_many("read_by_users", "users", "read_by");

  assert_eq!(rel.name, "read_by_users");
  assert_eq!(rel.relation_type, RelationType::ManyToMany);
  assert_eq!(rel.join_field, Some("read_by".to_string()));
  assert!(rel.local_key_in_array.is_none());
}

#[test]
fn test_relation_def_transform_map() {
  let rel =
    RelationDef::many_to_one("user", "users", "user_id").transform_map("userId", "profiles", "id");

  assert!(rel.transform_map_via.is_some());
  let tm = rel.transform_map_via.as_ref().unwrap();
  assert_eq!(tm.lookup_key, "userId");
  assert_eq!(tm.source_collection, "profiles");
  assert_eq!(tm.source_key, "id");
}

#[test]
fn test_relation_def_local_key_in_array() {
  let rel =
    RelationDef::many_to_one("members", "users", "member_ids").local_key_in_array("member_ids");

  assert!(rel.local_key_in_array.is_some());
  assert_eq!(rel.local_key_in_array.as_deref(), Some("member_ids"));
}

#[test]
fn test_relation_def_on_delete_cascade() {
  let rel = RelationDef::many_to_one("user", "users", "user_id").on_delete(SqlOnDelete::Cascade);

  assert_eq!(rel.on_delete, Some(SqlOnDelete::Cascade));
  assert!(rel.cascade_hard_delete);
  assert!(rel.cascade_soft_delete);
}

#[test]
fn test_relation_def_on_delete_restrict() {
  let rel = RelationDef::many_to_one("user", "users", "user_id").on_delete(SqlOnDelete::Restrict);

  assert_eq!(rel.on_delete, Some(SqlOnDelete::Restrict));
  assert!(rel.should_restrict());
}

#[test]
fn test_relation_def_on_delete_set_null() {
  let rel = RelationDef::many_to_one("user", "users", "user_id").on_delete(SqlOnDelete::SetNull);

  assert_eq!(rel.on_delete, Some(SqlOnDelete::SetNull));
  assert!(!rel.cascade_hard_delete);
}

#[test]
fn test_relation_registry_register_and_get() {
  clear_relation_registry();

  let relations = vec![
    RelationDef::one_to_many("posts", "posts", "author_id"),
    RelationDef::many_to_one("author", "users", "author_id"),
  ];
  register_collection_relations("articles", relations);

  let stored = get_collection_relations("articles");
  assert!(stored.is_some());
  assert_eq!(stored.unwrap().len(), 2);
}

#[test]
fn test_relation_registry_get_unknown_collection() {
  clear_relation_registry();

  let result = get_collection_relations("nonexistent");
  assert!(result.is_none());
}

#[test]
fn test_relation_registry_clear() {
  clear_relation_registry();

  let relations = vec![RelationDef::one_to_many("posts", "posts", "author_id")];
  register_collection_relations("articles", relations);

  clear_relation_registry();

  let result = get_collection_relations("articles");
  assert!(result.is_none());
}

#[test]
fn test_relation_def_should_cascade_hard_delete() {
  let rel_cascade =
    RelationDef::one_to_many("posts", "posts", "author_id").on_delete(SqlOnDelete::Cascade);
  assert!(rel_cascade.should_cascade_hard_delete());

  let rel_no_cascade = RelationDef::one_to_many("posts", "posts", "author_id");
  assert!(!rel_no_cascade.should_cascade_hard_delete());
}

#[test]
fn test_relation_def_should_cascade_soft_delete() {
  let rel_cascade =
    RelationDef::one_to_many("posts", "posts", "author_id").on_delete(SqlOnDelete::Cascade);
  assert!(rel_cascade.should_cascade_soft_delete());

  let rel_no_cascade = RelationDef::one_to_many("posts", "posts", "author_id");
  assert!(!rel_no_cascade.should_cascade_soft_delete());
}
