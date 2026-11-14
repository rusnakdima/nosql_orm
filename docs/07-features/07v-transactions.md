# Transactions

> Status: **implemented** — `TransactionControl` on every provider ships in
> 0.10.0, and so do the `storage.transaction(|tx| …)` closure helper and
> savepoints (SQLite, PostgreSQL, MySQL).

ACID transactions where the backend supports them, and an honest
`NotSupported` answer where it does not — never an id that cannot undo
anything.

---

## Implemented API

```rust
use nosql_orm::provider::TransactionControl;

let tx = storage.begin_transaction().await?;
// ... writes ...
storage.commit_transaction(tx.clone()).await?;
// or
storage.rollback_transaction(tx).await?;

assert!(!storage.is_transaction_active().await?);
```

`Repository<E, P>` exposes the same four methods (see
`src/repository/transaction.rs`).

### Closure helper

```rust
let value = storage.transaction(|_tx| async move {
  storage.insert("tasks", doc).await?;
  Ok(42)                       // -> commit, returns 42
}).await?;                     // Err(...) -> rollback, error re-raised

// The closure's own error is what the caller sees; a failing rollback or
// commit is reported alongside it, never swallowed.
```

### Savepoints (nested transactions)

```rust
use nosql_orm::provider::SavepointControl;

let tx = storage.begin_transaction().await?;
storage.savepoint("step_1").await?;
// ... writes ...
storage.rollback_to_savepoint("step_1").await?;   // undo this step only
storage.release_savepoint("step_1").await?;
storage.commit_transaction(tx).await?;             // outer tx unaffected

// Closure form: RELEASE on Ok, ROLLBACK TO on Err, outer tx stays active.
let kept = storage
  .savepoint_transaction("step_1", || async { /* ... */ Ok(value) })
  .await?;
```

Savepoint names are interpolated into SQL, so only
`[A-Za-z_][A-Za-z0-9_]*` is accepted — anything else is `InvalidInput`
before a statement runs. Savepoint support requires an active transaction.

### Who supports what

| Provider | Answer | Mechanism |
|---|---|---|
| JSON (`json`) | ✅ transactions | single active tx, rollback restores prior state |
| SQLite | ✅ transactions + savepoints | one connection held for the tx (`Capability::Transactions`) |
| PostgreSQL | ✅ transactions + savepoints | dedicated pooled connection, `BEGIN`/`COMMIT`/`ROLLBACK` |
| MySQL | ✅ transactions + savepoints | dedicated pooled connection, `START TRANSACTION` |
| MongoDB | ⛔ `NotSupported` | would need a session; says so instead of pretending |
| Redis | ⛔ `NotSupported` | write-buffer `RedisTxStorage` is the alternative |

Advertised through `Capability::Transactions` in `capabilities()` — call
`storage.supports(Capability::Transactions)` before relying on it.

Tested by `tests/test_new_features.rs::test_json_transactions_commit` /
`_rollback`, the rollback suites in `tests/test_provider_conformance.rs`,
`tests/test_transaction_helper.rs` (closure helper) and
`tests/test_savepoints.rs` (savepoints, `sql-sqlite`).

## Coming soon

- Compensating actions for providers without native transactions.

## See also

- [03-provider.md](../03-provider.md) — providers
- [04-repository.md](../04-repository.md) — repository
