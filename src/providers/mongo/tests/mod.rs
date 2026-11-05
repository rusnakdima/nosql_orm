use crate::error::{OrmError, OrmResult};

#[ignore]
#[tokio::test]
async fn mongo_insert_requires_live_server() -> OrmResult<()> {
  // Needs a live MongoDB server, so there is nothing to assert offline.
  // Report the missing capability as an error instead of panicking.
  Err(OrmError::NotSupported(
    "mongo insert needs a live MongoDB server".to_string(),
  ))
}

#[ignore]
#[tokio::test]
async fn mongo_find_by_id_requires_live_server() -> OrmResult<()> {
  // Needs a live MongoDB server, so there is nothing to assert offline.
  // Report the missing capability as an error instead of panicking.
  Err(OrmError::NotSupported(
    "mongo find_by_id needs a live MongoDB server".to_string(),
  ))
}
