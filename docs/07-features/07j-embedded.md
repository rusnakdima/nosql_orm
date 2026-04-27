# Embedded Entities

Embedded value objects within entities.

---

## Embedded

```rust
pub struct Embedded<T> { value: T }

pub trait EmbedExt {
    fn embed(&self) -> Value;
}
```

## EmbeddedMeta

```rust
pub struct EmbeddedMeta {
    pub name: String,
    pub fields: Vec<FieldMeta>,
}
```

---

## Usage

```rust
use nosql_orm::embedded::Embedded;

#[derive(Debug, Serialize, Deserialize)]
pub struct Address {
    pub street: String,
    pub city: String,
    pub zip: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    pub id: Option<String>,
    pub name: String,
    pub address: Embedded<Address>,
}
```

See `src/embedded/embedder.rs` for implementation.