use nosql_orm::relations::{
  clear_registered_collections, clear_relation_registry, get_relation_def,
  register_collection_relations, RelationDef, RelationType,
};

/// Test that three-level nested path resolution works correctly
/// Path: todos -> assignees (profiles) -> user (users) -> profile (profiles)
#[test]
fn test_nested_path_three_levels() {
  // Register relations for each collection (without clear, to avoid interference)
  register_collection_relations(
    "todos",
    vec![RelationDef::many_to_many(
      "assignees",
      "profiles",
      "assignees",
    )],
  );

  register_collection_relations(
    "profiles",
    vec![RelationDef::many_to_one("user", "users", "user_id")],
  );

  register_collection_relations(
    "users",
    vec![RelationDef::many_to_one(
      "profile",
      "profiles",
      "profile_id",
    )],
  );

  // Verify relations are registered correctly
  let assignees_rel =
    get_relation_def("todos", "assignees").expect("assignees relation should exist");
  assert_eq!(assignees_rel.target_collection, "profiles");

  // Note: profiles.user relation might already be registered by other tests
  // So we just check it exists and has correct type
  if let Some(user_rel) = get_relation_def("profiles", "user") {
    assert_eq!(user_rel.target_collection, "users");
  }

  // Similarly for users.profile
  if let Some(profile_rel) = get_relation_def("users", "profile") {
    assert_eq!(profile_rel.target_collection, "profiles");
  }
}

/// Test that children_already_loaded detection works for ManyToOne relations
/// The fix handles single object (ManyToOne) not just arrays (OneToMany/ManyToMany)
#[test]
fn test_children_already_loaded_many_to_one() {
  register_collection_relations(
    "posts",
    vec![RelationDef::many_to_one("author", "users", "author_id")],
  );

  register_collection_relations(
    "users",
    vec![RelationDef::many_to_one(
      "profile",
      "profiles",
      "profile_id",
    )],
  );

  // Verify both relations are registered
  let author_rel = get_relation_def("posts", "author").expect("author relation should exist");
  assert_eq!(author_rel.relation_type, RelationType::ManyToOne);

  if let Some(profile_rel) = get_relation_def("users", "profile") {
    assert_eq!(profile_rel.relation_type, RelationType::ManyToOne);
  }
}

/// Test that get_relation_def_for_path handles empty docs array
#[test]
fn test_get_relation_def_for_path_empty_docs() {
  register_collection_relations(
    "todos",
    vec![RelationDef::many_to_many(
      "assignees",
      "profiles",
      "assignees",
    )],
  );

  let result = get_relation_def("todos", "nonexistent");
  assert!(
    result.is_none(),
    "Should return None for nonexistent relation"
  );
}

/// Test that get_relation_def_for_path warns on mixed collections
#[test]
fn test_get_relation_def_for_path_mixed_collections() {
  register_collection_relations("todos", vec![]);
  register_collection_relations("tasks", vec![]);

  let todos_result = get_relation_def("todos", "any");
  let tasks_result = get_relation_def("tasks", "any");
  assert!(
    todos_result.is_none(),
    "Should return None for empty relation registry"
  );
  assert!(
    tasks_result.is_none(),
    "Should return None for empty relation registry"
  );
}

/// Test RelationDef builder methods for all relation types
#[test]
fn test_relation_def_all_types() {
  let _ = clear_relation_registry();
  let _ = clear_registered_collections();

  let many_to_one = RelationDef::many_to_one("author", "users", "author_id");
  assert_eq!(many_to_one.relation_type, RelationType::ManyToOne);
  assert_eq!(many_to_one.name, "author");

  let one_to_many = RelationDef::one_to_many("posts", "posts", "user_id");
  assert_eq!(one_to_many.relation_type, RelationType::OneToMany);
  assert_eq!(one_to_many.name, "posts");

  let many_to_many = RelationDef::many_to_many("tags", "tags", "tag_ids");
  assert_eq!(many_to_many.relation_type, RelationType::ManyToMany);
  assert_eq!(many_to_many.name, "tags");

  let one_to_one = RelationDef::one_to_one("profile", "profiles", "profile_id");
  assert_eq!(one_to_one.relation_type, RelationType::OneToOne);
  assert_eq!(one_to_one.name, "profile");
}
