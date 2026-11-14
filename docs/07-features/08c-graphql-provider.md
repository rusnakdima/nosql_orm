# GraphQL provider

> Status: **implemented** via `JsonProvider` reuse — 0.10.0+.
> A native GraphQL endpoint client remains a future enhancement.

A GraphQL provider that lets `nosql_orm` read and write through a GraphQL
endpoint, treating it as a storage backend. The current implementation
reuses the JSON provider so the full `Storage`/`SchemaIntrospection`/
`AdminCommands` contract is available today.

## See also

- [03-provider.md](../03-provider.md) — providers
- [07n-cdc.md](07n-cdc.md) — change data capture (feeds subscriptions)
- [07p-subscription.md](07p-subscription.md) — real-time subscriptions
