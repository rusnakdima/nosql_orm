# Encryption at rest

> Status: **implemented (document-level)** — `EncryptedStorage<P>` +
> `StaticKeyProvider` ship in 0.10.0. An `#[encrypted]` derive attribute,
> KMS-backed key providers and searchable encryption are **coming soon**.

Transparent encryption of named fields on the way into storage, and
decryption on the way out.

---

## Implemented API

```rust
use std::sync::Arc;
use nosql_orm::encryption::{EncryptedStorage, KeyProvider, StaticKeyProvider};

let key = StaticKeyProvider::new(b"0123456789abcdef".to_vec())?;  // AES-256-GCM
let store = EncryptedStorage::new(
  inner,
  Arc::new(key) as Arc<dyn KeyProvider>,
  vec!["ssn".into(), "notes".into()],   // fields to encrypt
);

store.insert("people", doc).await?;      // listed fields written as "enc:..."
store.find_by_id("people", id).await?;   // decrypted on read
```

- `KeyProvider: Send + Sync` is the pluggable key abstraction;
  `StaticKeyProvider` implements it with an in-memory key
  (`new`, `from_base64`).
- Only `Value::String` fields listed in `fields` are encrypted; values are
  prefixed `enc:` so already-encrypted or non-listed fields pass through.
- Decryption is a best-effort `strip_prefix("enc:")` + AES-GCM open — a
  wrong key leaves the ciphertext rather than corrupting the value.

Tested by `tests/test_new_features.rs::test_encryption_roundtrip`.

## Coming soon

- `#[encrypted]` field attribute in `nosql_orm_derive`.
- Key providers for env vars and cloud KMS.
- Searchable (deterministic) encryption for indexed fields.

## See also

- [07b-validators.md](07b-validators.md) — validators
- [02-entity.md](../02-entity.md) — Entity trait
