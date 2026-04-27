# Table Inheritance

Single-table inheritance pattern.

---

## Inheritance

```rust
pub enum InheritanceType {
    SingleTable,
    Joined,
    Concrete,
}
```

## Discriminator

```rust
pub struct Discriminator {
    pub column: String,
    pub value: String,
}
```

## Usage

```rust
pub trait Inheritance: Entity {
    fn inheritance_type() -> InheritanceType;
    fn discriminator() -> Option<Discriminator>;
}
```

See `src/inheritance/` for implementation.