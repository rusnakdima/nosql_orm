use crate::error::OrmResult;

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
