use crate::error::OrmResult;
use std::collections::HashMap;

#[async_trait::async_trait]
pub trait Seeder: Send + Sync {
  fn name(&self) -> &str;
  async fn run(&self) -> OrmResult<()>;
}

pub struct SeederRegistry {
  seeders: HashMap<String, Box<dyn Seeder>>,
}

impl Default for SeederRegistry {
  fn default() -> Self {
    Self::new()
  }
}

impl SeederRegistry {
  pub fn new() -> Self {
    Self {
      seeders: HashMap::new(),
    }
  }

  pub fn register<S: Seeder + 'static>(&mut self, seeder: S) {
    self
      .seeders
      .insert(seeder.name().to_string(), Box::new(seeder));
  }

  pub async fn run(&self, name: Option<&str>) -> OrmResult<()> {
    match name {
      Some(n) => {
        if let Some(seeder) = self.seeders.get(n) {
          seeder.run().await?;
        } else {
          println!("Seeder '{}' not found", n);
        }
      }
      None => {
        for seeder in self.seeders.values() {
          seeder.run().await?;
        }
      }
    }
    Ok(())
  }
}

pub struct FnSeeder<F> {
  pub name: String,
  pub func: F,
}

impl<F> FnSeeder<F>
where
  F: Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = OrmResult<()>> + Send + 'static>>
    + Send
    + Sync
    + 'static,
{
  pub fn new(name: String, func: F) -> Self {
    Self { name, func }
  }
}

#[async_trait::async_trait]
impl<F> Seeder for FnSeeder<F>
where
  F: Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = OrmResult<()>> + Send + 'static>>
    + Send
    + Sync
    + 'static,
{
  fn name(&self) -> &str {
    &self.name
  }

  async fn run(&self) -> OrmResult<()> {
    (self.func)().await
  }
}
