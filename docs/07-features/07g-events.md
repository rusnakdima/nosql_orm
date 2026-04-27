# Events

Entity event listeners for CRUD hooks.

---

## EventType

```rust
pub enum EventType {
    PreInsert,
    PostInsert,
    PreUpdate,
    PostUpdate,
    PreDelete,
    PostDelete,
}
```

## EntityEventListener

```rust
pub trait EntityEventListener<E: Entity>: Send + Sync {
    fn on_event(&self, event: Event<E>) -> OrmResult<()>;
}
```

## Event

```rust
pub struct Event<E: Entity> {
    pub event_type: EventType,
    pub entity: E,
}
```

---

## Example

```rust
use nosql_orm::events::{EntityEventListener, Event, EventType};

struct AuditListener;

impl<E: Entity> EntityEventListener<E> for AuditListener {
    fn on_event(&self, event: Event<E>) -> OrmResult<()> {
        match event.event_type {
            EventType::PreInsert => println!("About to insert"),
            EventType::PostInsert => println!("Inserted"),
            _ => {}
        }
        Ok(())
    }
}
```