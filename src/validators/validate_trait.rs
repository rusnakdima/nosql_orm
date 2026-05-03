use crate::error::OrmResult;

/// Trait for validating entity instances.
///
/// Implement this trait to add custom validation logic to your entities.
/// Return `Ok(())` if validation passes, or an `OrmResult::Err` with
/// `OrmError::Validation` if validation fails.
///
/// # Examples
///
/// ```
/// use nosql_orm::Validate;
/// use nosql_orm::error::OrmResult;
///
/// struct User {
///     name: String,
///     age: u32,
/// }
///
/// impl Validate for User {
///     fn validate(&self) -> OrmResult<()> {
///         if self.name.is_empty() {
///             return Err(Box::new(nosql_orm::error::OrmError::Validation("Name required".to_string())));
///         }
///         if self.age < 18 {
///             return Err(Box::new(nosql_orm::error::OrmError::Validation("Must be 18+".to_string())));
///         }
///         Ok(())
///     }
/// }
/// ```
pub trait Validate {
  fn validate(&self) -> OrmResult<()>;
}

impl<T: Validate> Validate for Option<T> {
  fn validate(&self) -> OrmResult<()> {
    if let Some(ref v) = self {
      v.validate()?;
    }
    Ok(())
  }
}

impl<T: Validate> Validate for Vec<T> {
  fn validate(&self) -> OrmResult<()> {
    for item in self {
      item.validate()?;
    }
    Ok(())
  }
}
