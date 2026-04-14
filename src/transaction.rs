use crate::entity::Entity;
use crate::error::{OrmError, OrmResult};
use crate::provider::DatabaseProvider;
use crate::repository::Repository;
use std::sync::Arc;
use std::sync::Mutex;

/// Transaction state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionState {
  Pending,
  Committed,
  RolledBack,
}

/// Transaction wrapper that tracks state and wraps a provider.
pub struct Transaction<P: DatabaseProvider> {
  #[allow(dead_code)]
  provider: Arc<P>,
  state: Mutex<TransactionState>,
}

impl<P: DatabaseProvider> Transaction<P> {
  /// Begin a new transaction with the given provider.
  pub async fn begin(provider: P) -> OrmResult<Self> {
    Ok(Self {
      provider: Arc::new(provider),
      state: Mutex::new(TransactionState::Pending),
    })
  }

  /// Commit the transaction, changing state to Committed.
  pub async fn commit(&mut self) -> OrmResult<()> {
    let mut state = self.state.lock().unwrap();
    match *state {
      TransactionState::Pending => {
        *state = TransactionState::Committed;
        Ok(())
      }
      TransactionState::Committed => Err(OrmError::Transaction(
        "Transaction already committed".to_string(),
      )),
      TransactionState::RolledBack => Err(OrmError::Transaction(
        "Transaction already rolled back".to_string(),
      )),
    }
  }

  /// Roll back the transaction, changing state to RolledBack.
  pub async fn rollback(&mut self) -> OrmResult<()> {
    let mut state = self.state.lock().unwrap();
    match *state {
      TransactionState::Pending => {
        *state = TransactionState::RolledBack;
        Ok(())
      }
      TransactionState::Committed => Err(OrmError::Transaction(
        "Transaction already committed".to_string(),
      )),
      TransactionState::RolledBack => Err(OrmError::Transaction(
        "Transaction already rolled back".to_string(),
      )),
    }
  }

  /// Get the current state of the transaction.
  pub fn state(&self) -> TransactionState {
    *self.state.lock().unwrap()
  }
}

impl<E, P> Repository<E, P>
where
  E: Entity,
  P: DatabaseProvider,
{
  /// Execute a closure within a transaction.
  pub async fn with_transaction<F, R>(&self, f: F) -> OrmResult<R>
  where
    F: FnOnce(&Transaction<P>) -> R,
    R: std::future::Future<Output = OrmResult<R>>,
  {
    let mut tx = Transaction::begin(self.provider.clone()).await?;
    let result = f(&tx).await;
    if result.is_ok() {
      tx.commit().await?;
    } else {
      let _ = tx.rollback().await;
    }
    result
  }
}
