use chrono::{DateTime, Utc};
use nosql_orm::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, nosql_orm::Model)]
#[table_name("test_users")]
#[soft_delete]
pub struct TestUser {
  pub id: Option<String>,
  pub name: String,
  pub email: String,
  pub deleted_at: Option<DateTime<Utc>>,
}

fn main() {
  println!("TestUser table_name: {}", TestUser::table_name());
  println!(
    "TestUser is_soft_deletable: {}",
    TestUser::is_soft_deletable()
  );
}
