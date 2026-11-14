# GraphQL provider (coming soon)

> Status: **coming soon** — planned for 0.12.0. Not implemented in 0.9.0.

A GraphQL provider that lets `nosql_orm` read and write through a GraphQL
endpoint, treating it as a storage backend. Also enables a generated GraphQL
API over any existing provider.

---

## Planned capabilities

### 1. GraphQL as a storage backend

```rust
let storage = GraphQLProvider::new("https://api.example.com/graphql", auth)?;
let repo = Repository::<User, _>::new(storage);
```

- Translate `Storage` CRUD calls into GraphQL queries/mutations.
- Schema introspection to discover types and fields.
- Auth via bearer token / API key / custom headers.

### 2. Generated GraphQL API over existing data

- Auto-generate a GraphQL schema from entity definitions.
- Query resolvers backed by the active provider (JSON, Mongo, SQL, …).
- Mutation resolvers for CRUD.
- Subscription resolvers wired to CDC (coming soon).

## Planned providers / integrations

| Integration | Notes |
|---|---|
| Apollo Server | Rust `async-graphql` + `axum`/`actix` |
| Hasura | Postgres-backed GraphQL; reuse SQL provider |
| PostGraphile | Auto-generated from Postgres schema |
| Custom endpoint | Any GraphQL endpoint as storage backend |

## Why coming soon

Requires a GraphQL client (for backend mode) and a schema generator + resolver
layer (for server mode). Tracked as `nosql_orm` roadmap item.

## See also

- [03-provider.md](../03-provider.md) — providers
- [07n-cdc.md](07n-cdc.md) — change data capture (coming soon, feeds subscriptions)
- [07p-subscription.md](07p-subscription.md) — real-time subscriptions (coming soon)
