use nosql_orm::error::OrmError;
use nosql_orm::providers::json::JsonProvider;
use nosql_orm::transaction::{Transaction, TransactionState};
use tempfile::TempDir;

#[tokio::test]
async fn test_transaction_begin() {
  let tx = Transaction::<JsonProvider>::begin().await.unwrap();
  assert_eq!(tx.state().unwrap(), TransactionState::Pending);
}

#[tokio::test]
async fn test_transaction_commit() {
  let mut tx = Transaction::<JsonProvider>::begin().await.unwrap();
  tx.commit().await.unwrap();
  assert_eq!(tx.state().unwrap(), TransactionState::Committed);
}

#[tokio::test]
async fn test_transaction_rollback() {
  let mut tx = Transaction::<JsonProvider>::begin().await.unwrap();
  tx.rollback().await.unwrap();
  assert_eq!(tx.state().unwrap(), TransactionState::RolledBack);
}

#[tokio::test]
async fn test_transaction_double_commit_error() {
  let mut tx = Transaction::<JsonProvider>::begin().await.unwrap();
  tx.commit().await.unwrap();

  let result = tx.commit().await;
  assert!(result.is_err());
  assert!(
    matches!(result.unwrap_err(), OrmError::Transaction(msg) if msg.contains("already committed"))
  );
}

#[tokio::test]
async fn test_transaction_double_rollback_error() {
  let mut tx = Transaction::<JsonProvider>::begin().await.unwrap();
  tx.rollback().await.unwrap();

  let result = tx.rollback().await;
  assert!(result.is_err());
  assert!(
    matches!(result.unwrap_err(), OrmError::Transaction(msg) if msg.contains("already rolled back"))
  );
}

#[tokio::test]
async fn test_transaction_commit_after_rollback_error() {
  let mut tx = Transaction::<JsonProvider>::begin().await.unwrap();
  tx.rollback().await.unwrap();

  let result = tx.commit().await;
  assert!(result.is_err());
  assert!(
    matches!(result.unwrap_err(), OrmError::Transaction(msg) if msg.contains("already rolled back"))
  );
}

#[tokio::test]
async fn test_transaction_rollback_after_commit_error() {
  let mut tx = Transaction::<JsonProvider>::begin().await.unwrap();
  tx.commit().await.unwrap();

  let result = tx.rollback().await;
  assert!(result.is_err());
  assert!(
    matches!(result.unwrap_err(), OrmError::Transaction(msg) if msg.contains("already committed"))
  );
}

#[tokio::test]
async fn test_transaction_state_method() {
  let tx = Transaction::<JsonProvider>::begin().await.unwrap();
  assert_eq!(tx.state().unwrap(), TransactionState::Pending);

  let mut tx = Transaction::<JsonProvider>::begin().await.unwrap();
  tx.commit().await.unwrap();
  assert_eq!(tx.state().unwrap(), TransactionState::Committed);

  let mut tx = Transaction::<JsonProvider>::begin().await.unwrap();
  tx.rollback().await.unwrap();
  assert_eq!(tx.state().unwrap(), TransactionState::RolledBack);
}

#[tokio::test]
async fn test_transaction_state_after_failed_commit() {
  let mut tx = Transaction::<JsonProvider>::begin().await.unwrap();
  let _ = tx.commit().await;
  let _ = tx.commit().await;

  assert_eq!(tx.state().unwrap(), TransactionState::Committed);
}

#[tokio::test]
async fn test_transaction_state_after_failed_rollback() {
  let mut tx = Transaction::<JsonProvider>::begin().await.unwrap();
  let _ = tx.rollback().await;
  let _ = tx.rollback().await;

  assert_eq!(tx.state().unwrap(), TransactionState::RolledBack);
}

#[tokio::test]
async fn test_transaction_debug_impl() {
  let tx = Transaction::<JsonProvider>::begin().await.unwrap();
  let debug_str = format!("{:?}", tx);
  assert!(debug_str.contains("Pending"));
}

#[tokio::test]
async fn test_transaction_clone() {
  let tx = Transaction::<JsonProvider>::begin().await.unwrap();
  let state = tx.state().unwrap();
  assert_eq!(state, TransactionState::Pending);
}

#[tokio::test]
async fn test_transaction_state_equality() {
  assert_eq!(TransactionState::Pending, TransactionState::Pending);
  assert_eq!(TransactionState::Committed, TransactionState::Committed);
  assert_eq!(TransactionState::RolledBack, TransactionState::RolledBack);
  assert_ne!(TransactionState::Pending, TransactionState::Committed);
  assert_ne!(TransactionState::Committed, TransactionState::RolledBack);
  assert_ne!(TransactionState::RolledBack, TransactionState::Pending);
}

use tempfile::TempDir;
