# Transactions

> Status: **implemented** — `TransactionControl` on every provider ships in
> 0.10.0. Savepoints and nested transactions are **coming soon**.

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

### Who supports what

| Provider | Answer | Mechanism |
|---|---|---|
| JSON (`json`) | ✅ transactions | single active tx, rollback restores prior state |
| SQLite | ✅ transactions | one connection held for the tx (`Capability::Transactions`) |
| PostgreSQL | ✅ transactions | dedicated pooled connection, `BEGIN`/`COMMIT`/`ROLLBACK` |
| MySQL | ✅ transactions | dedicated pooled connection, `START TRANSACTION` |
| MongoDB | ⛔ `NotSupported` | would need a session; says so instead of pretending |
| Redis | ⛔ `NotSupported` | write-buffer `RedisTxStorage` is the alternative |

Advertised through `Capability::Transactions` in `capabilities()` — call
`storage.supports(Capability::Transactions)` before relying on it.

Tested by `tests/test_new_features.rs::test_json_transactions_commit` /
`_rollback` and the rollback suites in
`tests/test_provider_conformance.rs`.

## Coming soon

- Savepoints for nested transactions.
- A `storage.transaction(|tx| async { ... })` closure helper that commits on
  `Ok` and rolls back on `Err`.
- Compensating actions for providers without native transactions.

## See also

- [03-provider.md](../03-provider.md) — providers
- [04-repository.md](../04-repository.md) — repository
