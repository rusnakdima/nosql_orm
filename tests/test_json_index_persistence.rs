//! Characterization test: a JsonProvider index declaration is bookkeeping
//! that must survive create/drop and a provider restart. Before this test the
//! three methods were no-ops and a declared index was silently discarded.

use tempfile::TempDir;

use nosql_orm::nosql_index::NosqlIndex;
use nosql_orm::provider::Storage;
use nosql_orm::providers::json::JsonProvider;

#[tokio::test]
async fn declared_indexes_round_trip_through_create_drop_and_restart() {
  let temp = TempDir::new().unwrap();
  let provider = JsonProvider::new(temp.path()).await.unwrap();

  assert!(
    provider.list_indexes("items").await.unwrap().is_empty(),
    "a fresh collection has no declared indexes"
  );

  provider
    .create_index(
      "items",
      &NosqlIndex::single("email", 1).name("idx_email").unique(),
    )
    .await
    .unwrap();
  provider
    .create_index("items", &NosqlIndex::single("name", 1).name("idx_name"))
    .await
    .unwrap();

  let listed = provider.list_indexes("items").await.unwrap();
  assert_eq!(listed.len(), 2, "both declared indexes are reported");
  let email = listed.iter().find(|i| i.name == "idx_email").unwrap();
  assert_eq!(email.fields, vec!["email".to_string()]);
  assert!(email.unique);
  assert_eq!(email.collection, "items");

  // Persistence: a new provider over the same directory sees the declarations.
  let reopened = JsonProvider::new(temp.path()).await.unwrap();
  assert_eq!(reopened.list_indexes("items").await.unwrap().len(), 2);

  // Drop removes exactly one and survives a restart too.
  provider.drop_index("items", "idx_email").await.unwrap();
  assert_eq!(provider.list_indexes("items").await.unwrap().len(), 1);
  let reopened = JsonProvider::new(temp.path()).await.unwrap();
  let remaining = reopened.list_indexes("items").await.unwrap();
  assert_eq!(remaining.len(), 1);
  assert_eq!(remaining[0].name, "idx_name");
}

#[tokio::test]
async fn indexes_are_per_collection() {
  let temp = TempDir::new().unwrap();
  let provider = JsonProvider::new(temp.path()).await.unwrap();

  provider
    .create_index(
      "users",
      &NosqlIndex::single("email", 1).name("idx_users_email"),
    )
    .await
    .unwrap();

  assert_eq!(provider.list_indexes("users").await.unwrap().len(), 1);
  assert!(
    provider.list_indexes("orders").await.unwrap().is_empty(),
    "an index on one collection is not reported on another"
  );
}
