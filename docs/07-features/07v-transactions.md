# Transactions (coming soon)

> Status: **coming soon** — planned for 0.10.0. Not implemented in 0.9.0.

ACID transactions across multiple operations and (where supported) multiple
collections, with rollback on failure.

---

## Planned API

```rust
storage.transaction(|tx| async {
    tx.insert("tasks", &task).await?;
    tx.insert("audit", &entry).await?;
    Ok(())
}).await?;
```

## Planned features

- Single-collection and multi-collection transactions.
- Provider-native where supported (Mongo sessions, SQL BEGIN/COMMIT).
- Best-effort compensating actions for providers without native
  transactions (JSON/Redis).
- Savepoints for nested transactions.

## Why coming soon

Requires a transaction handle abstraction over heterogeneous providers.
Tracked as `nosql_orm` roadmap item.

## See also

- [03-provider.md](../03-provider.md) — providers
- [04-repository.md](../04-repository.md) — repository
EOF