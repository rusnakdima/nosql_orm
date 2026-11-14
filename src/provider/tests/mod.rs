#[cfg(feature = "json")]
mod json {
  use crate::entity::{Entity, EntityMeta};
  use crate::error::{OrmError, OrmResult};
  use crate::nosql_index::{IndexManager, NosqlIndex};
  use crate::provider::{Capability, Storage, TransactionControl};
  use crate::providers::json::JsonProvider;
  use serde::{Deserialize, Serialize};

  #[derive(Debug, Clone, Serialize, Deserialize)]
  struct TestRecord {
    id: Option<String>,
    name: String,
  }

  impl Entity for TestRecord {
    fn meta() -> EntityMeta {
      EntityMeta::new("test_records")
    }

    fn get_id(&self) -> Option<String> {
      self.id.clone()
    }

    fn set_id(&mut self, id: String) {
      self.id = Some(id);
    }
  }

  impl crate::validators::Validate for TestRecord {
    fn validate(&self) -> OrmResult<()> {
      Ok(())
    }
  }

  fn expect_not_supported<T>(result: OrmResult<T>) {
    match result {
      Err(OrmError::NotSupported(_)) => {}
      other => panic!(
        "expected NotSupported, got {:?}",
        other.map(|_| "Ok" as &str)
      ),
    }
  }

  #[tokio::test]
  async fn capabilities_match_its_behaviour() {
    let dir = tempfile::tempdir().expect("temp dir");
    let provider = JsonProvider::new(dir.path()).await.expect("json provider");

    assert!(!provider.supports(Capability::Indexes));
    assert!(provider.supports(Capability::Transactions));
    assert!(provider.supports(Capability::Aggregation));
    assert!(provider.capabilities().names().contains(&"transactions"));

    let index = NosqlIndex::single("name", 1).name("idx_test_records_name");
    provider
    .create_index("test_records", &index)
    .await
    .expect("create_index is a no-op");
    provider
      .drop_index("test_records", "idx_test_records_name")
      .await
      .expect("drop_index is a no-op");
    let listed = Storage::list_indexes(&provider, "test_records")
      .await
      .expect("list_indexes is a no-op");
    assert!(listed.is_empty());
    // Transactions are now supported for the JSON provider.
    let tx_id = provider.begin_transaction().await.expect("begin_transaction");
    provider.commit_transaction(tx_id).await.expect("commit_transaction");
  }

  #[tokio::test]
  async fn index_manager_reports_not_supported() {
    let dir = tempfile::tempdir().expect("temp dir");
    let provider = JsonProvider::new(dir.path()).await.expect("json provider");
    let manager = IndexManager::new(provider);

    expect_not_supported(manager.drop_all_indexes("test_records").await);
    expect_not_supported(manager.sync_from_entity::<TestRecord>("test_records").await);
  }

  #[tokio::test]
  async fn backend_bridge_powers_the_repository() {
    let dir = tempfile::tempdir().expect("temp dir");
    let backend = crate::providers::backend::connect_backend(
      crate::providers::BackendKind::Json,
      dir.path().to_str().expect("temp path"),
    )
    .await
    .expect("json backend");

    let repository = crate::repository::Repository::<TestRecord, _>::new(backend);
    let stored = repository
      .insert(TestRecord {
        id: None,
        name: "bridge".to_string(),
      })
      .await
      .expect("insert through the bridge");

    let id = stored.id.expect("generated id");
    let found = repository
      .find_by_id(&id)
      .await
      .expect("find through the bridge")
      .expect("stored record");
    assert_eq!(found.name, "bridge");
  }
}

#[cfg(feature = "sql-sqlite")]
mod sqlite {
  use crate::nosql_index::NosqlIndex;
  use crate::provider::{AdminCommands, Capability, Storage, TransactionControl};
  use crate::providers::sql::SqliteProvider;

  async fn provider_with_table() -> SqliteProvider {
    let provider = SqliteProvider::connect(":memory:").await.expect("sqlite");
    provider
      .execute_raw(
        "CREATE TABLE test_records (id TEXT PRIMARY KEY, name TEXT)",
        vec![],
      )
      .await
      .expect("create table");
    provider
  }

  #[tokio::test]
  async fn capabilities_match_behaviour() {
    let provider = provider_with_table().await;

    assert!(provider.supports(Capability::Indexes));
    assert!(provider.supports(Capability::Transactions));
    assert!(provider.supports(Capability::RawQueries));
    assert!(!provider.supports(Capability::Aggregation));

    let index = NosqlIndex::single("name", 1).name("idx_test_records_name");
    provider
      .create_index("test_records", &index)
      .await
      .expect("create index");

    let indexes = Storage::list_indexes(&provider, "test_records")
      .await
      .expect("list indexes");
    let listed = indexes
      .iter()
      .find(|info| info.name == "idx_test_records_name")
      .expect("created index is listed");
    assert_eq!(listed.fields, vec!["name".to_string()]);
    assert!(!listed.unique);
    assert!(provider
      .index_exists("test_records", "idx_test_records_name")
      .await
      .expect("index exists"));

    provider
      .drop_index("test_records", "idx_test_records_name")
      .await
      .expect("drop index");
    assert!(!provider
      .index_exists("test_records", "idx_test_records_name")
      .await
      .expect("index gone"));
  }

  #[tokio::test]
  async fn rollback_discards_writes() {
    let provider = provider_with_table().await;

    let id = provider.begin_transaction().await.expect("begin");
    provider
      .insert(
        "test_records",
        serde_json::json!({"id": "rolled-back", "name": "discard"}),
      )
      .await
      .expect("insert in transaction");
    provider.rollback_transaction(id).await.expect("rollback");

    assert!(provider
      .find_by_id("test_records", "rolled-back")
      .await
      .expect("find")
      .is_none());
    assert!(!provider.is_transaction_active().await.expect("active flag"));
  }

  #[tokio::test]
  async fn commit_persists_writes() {
    let provider = provider_with_table().await;

    let id = provider.begin_transaction().await.expect("begin");
    provider
      .insert(
        "test_records",
        serde_json::json!({"id": "committed", "name": "keep"}),
      )
      .await
      .expect("insert in transaction");
    provider.commit_transaction(id).await.expect("commit");

    assert!(provider
      .find_by_id("test_records", "committed")
      .await
      .expect("find")
      .is_some());
    assert!(!provider.is_transaction_active().await.expect("active flag"));
  }
}
