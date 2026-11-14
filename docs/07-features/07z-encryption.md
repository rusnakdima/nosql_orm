# Encryption at rest (coming soon)

> Status: **coming soon** — planned for 0.11.0. Not implemented in 0.9.0.

Transparent field-level and document-level encryption for sensitive data,
with key management hooks.

---

## Planned API

```rust
#[derive(Entity)]
struct Secret {
    id: String,
    #[encrypted]
    value: String,
}
```

## Planned features

- `#[encrypted]` field attribute (derive macro).
- Pluggable cipher (AES-256-GCM default).
- Key provider trait (env, KMS, custom).
- Searchable encryption (deterministic for indexed fields).

## Why coming soon

Requires derive-macro support and a key-management abstraction. Tracked as
`nosql_orm` roadmap item.

## See also

- [07b-validators.md](07b-validators.md) — validators
- [02-entity.md](../02-entity.md) — Entity trait
