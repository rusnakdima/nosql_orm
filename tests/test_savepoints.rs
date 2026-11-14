// Savepoints and the savepoint closure helper on the SQLite provider
// (B3.4). Requires a real connection, hence sql-sqlite.

use nosql_orm::error::OrmError;
use nosql_orm::provider::{AdminCommands, SavepointControl, TransactionControl};
use nosql_orm::providers::sql::SqliteProvider;

async fn exec_ids(provider: &SqliteProvider) -> Vec<String> {
  let result = provider
    .execute_raw("SELECT id FROM sp_demo ORDER BY id", vec![])
    .await
    .unwrap();
  result
    .rows
    .into_iter()
    .map(|row| row[0].as_str().unwrap_or("").to_string())
    .collect()
}

async fn fresh_provider() -> SqliteProvider {
  let provider = SqliteProvider::connect(":memory:").await.unwrap();
  provider
    .execute_raw("CREATE TABLE sp_demo (id TEXT PRIMARY KEY, v TEXT)", vec![])
    .await
    .unwrap();
  provider
}

#[tokio::test]
async fn rollback_to_savepoint_discards_only_its_writes() {
  let provider = fresh_provider().await;

  let tx = provider.begin_transaction().await.unwrap();
  provider
    .execute_raw("INSERT INTO sp_demo VALUES ('a', 'first')", vec![])
    .await
    .unwrap();

  provider.savepoint("before_b").await.unwrap();
  provider
    .execute_raw("INSERT INTO sp_demo VALUES ('b', 'second')", vec![])
    .await
    .unwrap();
  provider.rollback_to_savepoint("before_b").await.unwrap();

  // The outer transaction survives the savepoint rollback.
  assert!(provider.is_transaction_active().await.unwrap());
  provider.commit_transaction(tx).await.unwrap();

  assert_eq!(exec_ids(&provider).await, vec!["a".to_string()]);
}

#[tokio::test]
async fn savepoint_transaction_helper_keeps_ok_and_discards_err() {
  let provider = fresh_provider().await;
  let tx = provider.begin_transaction().await.unwrap();

  let worker = provider.clone();
  let value = provider
    .savepoint_transaction("kept", move || async move {
      worker
        .execute_raw("INSERT INTO sp_demo VALUES ('ok', 'kept')", vec![])
        .await?;
      Ok(7)
    })
    .await
    .unwrap();
  assert_eq!(value, 7);

  let worker = provider.clone();
  let outcome: Result<(), OrmError> = provider
    .savepoint_transaction("doomed", move || async move {
      worker
        .execute_raw("INSERT INTO sp_demo VALUES ('bad', 'doomed')", vec![])
        .await?;
      Err(OrmError::InvalidInput("nope".to_string()))
    })
    .await;
  assert!(matches!(outcome.unwrap_err(), OrmError::InvalidInput(_)));

  // The outer transaction is still open after the nested rollback.
  assert!(provider.is_transaction_active().await.unwrap());
  provider.commit_transaction(tx).await.unwrap();

  // Only the released write survived.
  assert_eq!(exec_ids(&provider).await, vec!["ok".to_string()]);
}

#[tokio::test]
async fn savepoint_names_are_validated_before_touching_sql() {
  let provider = fresh_provider().await;

  // No active transaction yet: the provider refuses first.
  let err = provider.savepoint("fine_name").await.unwrap_err();
  assert!(matches!(err, OrmError::Transaction(_)));

  let tx = provider.begin_transaction().await.unwrap();
  for bad in ["bad; DROP TABLE sp_demo", "1lead", "", "with space"] {
    let err = provider.savepoint(bad).await.unwrap_err();
    assert!(
      matches!(err, OrmError::InvalidInput(_)),
      "expected InvalidInput for {:?}, got {:?}",
      bad,
      err
    );
  }

  let err = provider
    .savepoint_transaction("bad name", || async { Ok::<(), OrmError>(()) })
    .await
    .unwrap_err();
  assert!(matches!(err, OrmError::InvalidInput(_)));

  provider.rollback_transaction(tx).await.unwrap();
  assert!(!provider.is_transaction_active().await.unwrap());
}
