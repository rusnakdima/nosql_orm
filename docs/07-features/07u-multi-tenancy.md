# Multi-tenancy

> Status: **implemented** — `TenantStorage<P>` ships in 0.10.0.
> Tenant-aware migrations and an admin tenant API are **coming soon**.

Transparent tenant isolation: every repository operation is scoped to a
tenant id so one database can serve multiple apps/users without
cross-tenant leakage.

---

## Implemented API

```rust
use nosql_orm::tenant::TenantStorage;

let store = TenantStorage::new(inner, "tenant-a");
store.tenant_id();                       // "tenant-a"

// writes get a `_tenant_id` field injected
store.insert("tasks", doc).await?;
// every read is filtered to this tenant
store.find_many("tasks", None, None, None, None, true).await?;
```

`TenantStorage<P: Storage>` wraps any provider and forwards
`insert`, `find_by_id`, `find_many`, `update`, `patch`, `delete`,
`delete_many`, `update_many`, `count`, plus the index and
`SchemaIntrospection` methods, injecting `tenant_filter()` into every read
and `inject_tenant()` into every write.

Tested by `tests/test_new_features.rs::test_tenant_isolation`.

## Coming soon

- Tenant-aware migrations.
- Admin API to list/switch tenants.
- Per-tenant collection prefixes (today the discriminator is a field).

## See also

- [07e-migrations.md](07e-migrations.md) — migrations
- [07c-soft-delete.md](07c-soft-delete.md) — soft delete
