# Change Data Capture (CDC)

Track and stream data changes.

Opt-in: compiled only with the `cdc` feature. No code path in `Storage`,
`Repository` or the providers calls this module yet.

---

## ChangeType

```rust
pub enum ChangeType {
    Insert,
    Update,
    Delete,
}
```

## Change

```rust
pub struct Change {
    pub change_type: ChangeType,
    pub collection: String,
    pub document_key: String,
    pub full_document: Option<Value>,
    pub timestamp: DateTime<Utc>,
}
```

## ChangeStream

```rust
pub trait ChangeStream {
    async fn watch(&self, collection: &str) -> impl Stream<Item = OrmResult<Change>>;
}
```

## AuditLog

```rust
pub struct AuditLog {
    pub action: AuditAction,
    pub entity: String,
    pub entity_id: String,
    pub old_value: Option<Value>,
    pub new_value: Option<Value>,
    pub timestamp: DateTime<Utc>,
    pub user_id: Option<String>,
}
```