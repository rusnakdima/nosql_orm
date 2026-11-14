# Encryption at rest

> Status: **implemented (document-level + searchable)** —
> `EncryptedStorage<P>`, `StaticKeyProvider`, `EnvKeyProvider` and
> deterministic-HMAC searchable encryption ship in 0.10.0. An
> `#[encrypted]` derive attribute and a cloud KMS key provider are
> **coming soon**.

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

### Env-var key provider

```rust
use nosql_orm::encryption::EnvKeyProvider;

// Accepts base64 of 32 bytes, or a raw 32-byte value.
let key = EnvKeyProvider::from_env("MYAPP_ENC_KEY")?;
```

The local stand-in for a KMS: the key never lives in source, a secret
manager injects it into the environment. Missing or wrong-sized values
are `Encryption` errors.

### Searchable encryption

```rust
use nosql_orm::encryption::{token_field, EncryptedStorage};

let store = EncryptedStorage::new(inner, key, vec!["email".into()])
  .with_searchable(vec!["email".into()]);

// Equality on the plaintext now matches: the filter is rewritten to the
// deterministic token field behind the scenes.
let filter = Filter::Eq("email".into(), json!("alice@example.com"));
store.find_many("users", Some(&filter), ...).await?;

// Declare the index behind the lookup (index on `email__token`).
store.create_searchable_index("users", "email").await?;
```

- Each searchable field keeps a companion `field__token` holding a
  deterministic HMAC-SHA256 tag of the plaintext — equal plaintexts give
  equal tags, so equality (`Eq`, `In`, nested `And`/`Or`/`Not`) filters run
  against the tag instead of the ciphertext.
- The tag is stripped from every read result: callers see the decrypted
  field, never the storage detail.
- The value itself stays AES-256-GCM encrypted. The tag leaks *equality*
  (that is what an indexed lookup needs) but is not reversible without
  the key.
- `count`, `delete_many` and `update_many` rewrite searchable filters too,
  so a filter behaves the same across read and write paths.

Tested by `tests/test_searchable_encryption.rs`.

## Coming soon

- `#[encrypted]` field attribute in `nosql_orm_derive`.
- Cloud KMS key provider (AWS KMS / GCP KMS / Vault).

## See also

- [07b-validators.md](07b-validators.md) — validators
- [02-entity.md](../02-entity.md) — Entity trait
