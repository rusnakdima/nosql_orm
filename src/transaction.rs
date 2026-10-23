use crate::entity::Entity;
use crate::error::{OrmError, OrmResult};
use crate::provider::Storage;
use crate::repository::Repository;
use std::marker::PhantomData;
use std::sync::Mutex;

/// Transaction state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionState {
  Pending,
  Committed,
  RolledBack,
}

/// Transaction wrapper that tracks state and wraps a provider.
pub struct Transaction<P: Storage> {
  state: Mutex<TransactionState>,
  _provider: PhantomData<P>,
}

impl<P: Storage> Transaction<P> {
  pub async fn begin() -> OrmResult<Self> {
    Ok(Self {
      state: Mutex::new(TransactionState::Pending),
      _provider: PhantomData,
    })
  }

  /// Commit the transaction, changing state to Committed.
  pub async fn commit(&mut self) -> OrmResult<()> {
    let mut state = self
      .state
      .lock()
      .map_err(|e| OrmError::Transaction(format!("Lock poisoned: {}", e)))?;
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
    let mut state = self
      .state
      .lock()
      .map_err(|e| OrmError::Transaction(format!("Lock poisoned: {}", e)))?;
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
  pub fn state(&self) -> OrmResult<TransactionState> {
    let guard = self
      .state
      .lock()
      .map_err(|e| OrmError::Transaction(format!("Lock poisoned: {}", e)))?;
    Ok(*guard)
  }
}

impl<P: Storage> std::fmt::Debug for Transaction<P> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let state = self.state.lock().map_err(|_| std::fmt::Error)?;
    f.debug_struct("Transaction")
      .field("state", &*state)
      .finish()
  }
}

impl<E, P> Repository<E, P>
where
  E: Entity,
  P: Storage,
{
  /// Execute a closure within a transaction.
  pub async fn with_transaction<F, R>(&self, f: F) -> OrmResult<R>
  where
    F: FnOnce(&Transaction<P>) -> R,
    R: std::future::Future<Output = OrmResult<R>>,
  {
    let mut tx = Transaction::<P>::begin().await?;
    let result = f(&tx).await;
    if result.is_ok() {
      tx.commit().await?;
    } else {
      let _ = tx.rollback().await;
    }
    result
  }
}
