use super::*;
use serde_json::json;

fn fixture() -> (StructureManifest, BTreeMap<String, Vec<Value>>) {
  let manifest = StructureManifest::parse(
    r#"{
      "schema_version":"1.0.0", "kind":"forge.structure", "mode":"report",
      "collections":{
        "parents":{
          "primary_key":["id"],
          "fields":{
            "id":{"type":"string","required":true,"unique":true},
            "code":{"type":"string","unique":true},
            "owner_id":{"type":"relation","required":true,"ref":"owners.id"},
            "status":{"type":"string","enum":"Status"},
            "count_children":{"type":"int","derived":"count(children.parent_id)"},
            "dupes":{"type":"array"}
          },
          "unique":[{"name":"uq_parent_code_owner","fields":["code","owner_id"]}],
          "owned_children":[{"collection":"children","via":"parent_id","parent_field":"child_ids"}],
          "forbidden_repeating_groups":["dupes[]"]
        },
        "owners":{"primary_key":["id"],"fields":{"id":{"type":"string","required":true}}},
        "children":{"primary_key":["id"],"fields":{
          "id":{"type":"string","required":true},
          "parent_id":{"type":"relation","ref":"parents.id"}
        }}
      },
      "enums":{"Status":["open","closed"]},
      "indexes":[{"collection":"parents","name":"idx_parents_owner","fields":["owner_id"],"unique":false}]
    }"#,
  )
  .unwrap();
  let mut collections = BTreeMap::new();
  collections.insert(
    "owners".to_string(),
    vec![json!({"id":"o1"}), json!({"id":"o2"})],
  );
  collections.insert(
    "children".to_string(),
    vec![
      json!({"id":"c1","parent_id":"p1"}),
      json!({"id":"c2","parent_id":"p1"}),
    ],
  );
  collections.insert(
    "parents".to_string(),
    vec![json!({
      "id":"p1","code":"A","owner_id":"o1","status":"open",
      "count_children":2,"child_ids":["c1","c2"],"dupes":[]
    })],
  );
  (manifest, collections)
}

fn codes(findings: &[StructureViolation]) -> Vec<StructureViolationCode> {
  findings.iter().map(|v| v.code).collect()
}

#[test]
fn valid_normalized_content_has_no_blocking_findings() {
  let (manifest, collections) = fixture();
  let findings = validate_records(&manifest, &collections);
  // Only the declared derived cache is reported (3NF warning).
  assert_eq!(
    codes(&findings),
    vec![StructureViolationCode::TransitiveDependency]
  );
  assert_eq!(blocking_violations(&findings).len(), 0);
  assert_eq!(findings[0].severity, StructureSeverity::Warning);
}

#[test]
fn strict_mode_escalates_warnings_to_errors() {
  let (mut manifest, collections) = fixture();
  manifest.mode = StructureMode::Strict;
  let findings = validate_records(&manifest, &collections);
  assert!(findings
    .iter()
    .all(|v| v.severity == StructureSeverity::Error));
  assert_eq!(blocking_violations(&findings).len(), findings.len());
}

#[test]
fn duplicate_primary_key_is_named_and_located() {
  let (manifest, mut collections) = fixture();
  collections.get_mut("parents").unwrap().push(json!({
    "id":"p1","code":"B","owner_id":"o1","status":"open","count_children":0,"dupes":[]
  }));
  let findings = validate_records(&manifest, &collections);
  assert!(codes(&findings).contains(&StructureViolationCode::DuplicatePrimaryKey));
  let dup = findings
    .iter()
    .find(|v| v.code == StructureViolationCode::DuplicatePrimaryKey)
    .unwrap();
  assert_eq!(dup.collection, "parents");
  assert_eq!(dup.record, "p1");
  assert_eq!(dup.field.as_deref(), Some("id"));
}

#[test]
fn missing_required_field_is_reported() {
  let (manifest, mut collections) = fixture();
  collections.get_mut("parents").unwrap()[0]
    .as_object_mut()
    .unwrap()
    .remove("owner_id");
  let findings = validate_records(&manifest, &collections);
  assert!(findings.iter().any(|v| {
    v.code == StructureViolationCode::MissingRequired && v.field.as_deref() == Some("owner_id")
  }));
}

#[test]
fn broken_foreign_key_is_reported() {
  let (manifest, mut collections) = fixture();
  collections.get_mut("parents").unwrap()[0]["owner_id"] = json!("missing");
  let findings = validate_records(&manifest, &collections);
  assert!(findings.iter().any(|v| {
    v.code == StructureViolationCode::BrokenForeignKey
      && v.field.as_deref() == Some("owner_id")
      && v.record == "p1"
  }));
}

#[test]
fn field_and_compound_unique_violations_are_reported() {
  let (manifest, mut collections) = fixture();
  collections.get_mut("parents").unwrap().push(json!({
    "id":"p2","code":"A","owner_id":"o1","status":"open","count_children":0,"dupes":[]
  }));
  let findings = validate_records(&manifest, &collections);
  let uniques: Vec<_> = findings
    .iter()
    .filter(|v| v.code == StructureViolationCode::UniqueViolation)
    .collect();
  assert_eq!(
    uniques.len(),
    2,
    "field unique and compound unique both fire"
  );
}

#[test]
fn enum_and_type_violations_are_reported() {
  let (manifest, mut collections) = fixture();
  collections.get_mut("parents").unwrap()[0]["status"] = json!("");
  collections.get_mut("parents").unwrap()[0]["count_children"] = json!("not a number");
  let findings = validate_records(&manifest, &collections);
  assert!(codes(&findings).contains(&StructureViolationCode::EnumViolation));
  assert!(codes(&findings).contains(&StructureViolationCode::TypeMismatch));
}

#[test]
fn forbidden_repeating_group_is_reported_but_empty_array_is_fine() {
  let (manifest, mut collections) = fixture();
  collections.get_mut("parents").unwrap()[0]["dupes"] = json!([{"x":1}]);
  let findings = validate_records(&manifest, &collections);
  assert!(findings.iter().any(|v| {
    v.code == StructureViolationCode::ForbiddenRepeatingGroup && v.field.as_deref() == Some("dupes")
  }));
}

#[test]
fn owned_children_accept_single_parent_and_reject_duplication() {
  let (manifest, mut collections) = fixture();
  // Same child id under a second parent -> the "duplicated across parents" case.
  collections.get_mut("parents").unwrap().push(json!({
    "id":"p2","code":"B","owner_id":"o2","status":"closed",
    "count_children":0,"child_ids":["c1"],"dupes":[]
  }));
  let findings = validate_records(&manifest, &collections);
  assert!(findings
    .iter()
    .any(|v| { v.code == StructureViolationCode::DuplicatedOwnedChild && v.record == "p2" }));
}

#[test]
fn stale_derived_cache_is_reported() {
  let (manifest, mut collections) = fixture();
  collections.get_mut("parents").unwrap()[0]["count_children"] = json!(99);
  let findings = validate_records(&manifest, &collections);
  assert!(findings
    .iter()
    .any(|v| v.code == StructureViolationCode::DerivedStale));
}

#[test]
fn manifest_version_and_unknown_collections_are_validated() {
  assert!(matches!(
    StructureManifest::parse(r#"{"schema_version":"2.0.0"}"#),
    Err(StructureError::UnsupportedVersion(_))
  ));
  assert!(matches!(
    StructureManifest::parse(
      r#"{"schema_version":"1.0.0","collections":{"a":{"primary_key":["id"]}}}"#
    ),
    Err(StructureError::InvalidManifest(_))
  ));
}

#[tokio::test]
async fn json_provider_enforces_manifest_unique_and_required_on_write() {
  use crate::error::OrmError;
  use crate::provider::Storage;
  use crate::providers::json::{JsonProvider, JsonProviderConfig};

  let (manifest, _) = fixture();
  let dir = tempfile::tempdir().unwrap();
  let config = JsonProviderConfig::new(dir.path()).with_structure_manifest(manifest);
  let provider = JsonProvider::with_config(config).await.unwrap();
  // FK targets must exist in the provider snapshot too.
  provider.insert("owners", json!({"id":"o1"})).await.unwrap();
  provider.insert("owners", json!({"id":"o2"})).await.unwrap();

  let first = json!({
    "id":"p1","code":"A","owner_id":"o1","status":"open",
    "count_children":0,"child_ids":[],"dupes":[]
  });
  provider.insert("parents", first.clone()).await.unwrap();

  // Same PK -> Duplicate (historic behavior too).
  let duplicate_pk = provider.insert("parents", first.clone()).await.unwrap_err();
  assert!(
    matches!(duplicate_pk, OrmError::Duplicate(_)),
    "{duplicate_pk:?}"
  );

  // New PK, unique field collision -> Duplicate (new manifest enforcement).
  let duplicate_unique = json!({
    "id":"p2","code":"A","owner_id":"o2","status":"open",
    "count_children":0,"child_ids":[],"dupes":[]
  });
  let err = provider
    .insert("parents", duplicate_unique)
    .await
    .unwrap_err();
  assert!(matches!(err, OrmError::Duplicate(_)), "{err:?}");

  // Missing required field -> Validation, never a silent write.
  let incomplete = json!({"id":"p3","code":"C","status":"open"});
  let err = provider.insert("parents", incomplete).await.unwrap_err();
  assert!(matches!(err, OrmError::Validation(_)), "{err:?}");
  assert!(provider
    .find_by_id("parents", "p3")
    .await
    .unwrap()
    .is_none());
}

#[tokio::test]
async fn json_provider_without_a_manifest_keeps_historic_behavior() {
  use crate::error::OrmError;
  use crate::provider::Storage;
  use crate::providers::json::JsonProvider;

  let dir = tempfile::tempdir().unwrap();
  let provider = JsonProvider::new(dir.path()).await.unwrap();
  provider
    .insert("anything", json!({"id":"a","code":"same"}))
    .await
    .unwrap();
  // Field-level uniqueness is NOT enforced without a manifest entry.
  provider
    .insert("anything", json!({"id":"b","code":"same"}))
    .await
    .unwrap();
  // Duplicate PK always is.
  let err = provider
    .insert("anything", json!({"id":"a"}))
    .await
    .unwrap_err();
  assert!(matches!(err, OrmError::Duplicate(_)));
}

#[tokio::test]
async fn json_provider_declares_manifest_indexes() {
  use crate::provider::Storage;
  use crate::providers::json::{JsonProvider, JsonProviderConfig};

  let (manifest, _) = fixture();
  let dir = tempfile::tempdir().unwrap();
  let provider = JsonProvider::with_config(
    JsonProviderConfig::new(dir.path()).with_structure_manifest(manifest),
  )
  .await
  .unwrap();
  let created = provider.apply_structure_indexes().await.unwrap();
  assert!(created >= 3);
  let indexes = provider.list_indexes("parents").await.unwrap();
  assert!(indexes.iter().any(|i| i.name == "pk_parents" && i.unique));
}

#[test]
fn index_declarations_carry_unique_flags() {
  let (manifest, _) = fixture();
  let indexes = manifest.index_declarations();
  let names: Vec<&str> = indexes
    .iter()
    .map(|(_, idx)| idx.get_name().unwrap_or("unnamed"))
    .collect();
  assert!(names.contains(&"idx_parents_owner"));
  assert!(names.contains(&"pk_parents"));
  let pk = indexes
    .iter()
    .find(|(_, idx)| idx.get_name() == Some("pk_parents"))
    .unwrap();
  assert!(pk.1.is_unique());
  assert_eq!(manifest.unique_field_sets("parents").len(), 3);
}
