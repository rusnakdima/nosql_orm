# Timestamps

Auto-created `created_at` and `updated_at` timestamp fields.

---

## apply_timestamps()

```rust
pub fn apply_timestamps(doc: &mut Value, is_insert: bool) {
    let now = Utc::now();
    
    if is_insert {
        doc["created_at"] = json!(now.to_rfc3339());
    }
    doc["updated_at"] = json!(now.to_rfc3339());
}
```

## Usage

```rust
use nosql_orm::timestamps::apply_timestamps;

// In Repository::insert
let mut doc = entity.to_value()?;
apply_timestamps(&mut doc, true);  // Sets created_at AND updated_at

// In Repository::update  
let mut doc = entity.to_value()?;
apply_timestamps(&mut doc, false);  // Sets updated_at only
```

---

## Next Steps

- [07e-migrations.md](07e-migrations.md) - Migration system