# Multi-tenancy (coming soon)

> Status: **coming soon** — planned for 0.10.0. Not implemented in 0.9.0.

Transparent tenant isolation: every repository operation is scoped to a
tenant id so a single database can serve multiple apps/users without
cross-tenant leakage.

---

## Planned API

```rust
let repo = Repository::new(storage).tenant(tenant_id);
// all reads/writes scoped automatically
```

## Planned features

- Tenant id injected into every query filter automatically.
- Per-tenant collection prefix or row-level discriminator.
- Tenant-aware migrations.
- Admin API to list/switch tenants.

## Why coming soon

Needs a consistent tenant context propagation through Storage/Repository
and providers. Tracked as `nosql_orm` roadmap item.

## See also

- [07e-migrations.md](07e-migrations.md) — migrations
- [07c-soft-delete.md](07c-soft-delete.md) — soft delete
EOF