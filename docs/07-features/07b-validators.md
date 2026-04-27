# Validators

Entity field validation system.

---

## Validate Trait

```rust
pub trait Validate {
    fn validate(&self) -> OrmResult<()>;
}
```

## Built-in Validators

```rust
pub enum ValidatorType {
    Email,
    Uuid,
    Url,
    Length(Option<usize>, Option<usize>),
    Pattern(String),
    Range(Option<f64>, Option<f64>),
    Min(f64),
    Max(f64),
    NotEmpty,
    NonNull,
    Required,
}
```

## Using Validate

```rust
use nosql_orm::validators::Validate;

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    pub email: String,
    pub name: String,
    pub age: u32,
}

impl Validate for User {
    fn validate(&self) -> OrmResult<()> {
        if !self.email.contains('@') {
            return Err(OrmError::Validation("Invalid email".into()));
        }
        if self.name.is_empty() {
            return Err(OrmError::Validation("Name required".into()));
        }
        if self.age < 18 {
            return Err(OrmError::Validation("Must be 18+".into()));
        }
        Ok(())
    }
}

// Validate entity
user.validate()?;  // Returns Ok(()) or Err(OrmError)
```

## Using Validate Macro

```rust
use nosql_orm_derive::Validate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct User {
    #[validate(email)]
    pub email: String,
    
    #[validate(length(min = 2, max = 50))]
    pub name: String,
    
    #[validate(min = 18)]
    pub age: u32,
}
```

## Validation Wrappers

Validate is implemented for Option and Vec:

```rust
impl<T: Validate> Validate for Option<T> {
    fn validate(&self) -> OrmResult<()> {
        if let Some(ref v) = self { v.validate()?; }
        Ok(())
    }
}

impl<T: Validate> Validate for Vec<T> {
    fn validate(&self) -> OrmResult<()> {
        for item in self { item.validate()?; }
        Ok(())
    }
}
```

---

## Next Steps

- [07c-soft-delete.md](07c-soft-delete.md) - Soft deletes