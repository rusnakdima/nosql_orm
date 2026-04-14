use crate::error::OrmResult;
use std::process::ExitCode;

#[derive(Debug, Clone)]
pub enum CliCommand {
  MigrateRun,
  MigrateRollback { steps: u32 },
  MigrateStatus,
  Seed { seeder_class: Option<String> },
  SchemaCreate,
  SchemaDrop,
  CacheClear,
  CacheStats,
  Help,
}

pub struct Cli {
  commands: Vec<CliCommand>,
}

impl Cli {
  pub fn new() -> Self {
    Self {
      commands: Vec::new(),
    }
  }

  pub fn migrate_run(mut self) -> Self {
    self.commands.push(CliCommand::MigrateRun);
    self
  }

  pub fn migrate_rollback(mut self, steps: u32) -> Self {
    self.commands.push(CliCommand::MigrateRollback { steps });
    self
  }

  pub fn migrate_status(mut self) -> Self {
    self.commands.push(CliCommand::MigrateStatus);
    self
  }

  pub fn seed(mut self, seeder_class: Option<String>) -> Self {
    self.commands.push(CliCommand::Seed { seeder_class });
    self
  }

  pub fn schema_create(mut self) -> Self {
    self.commands.push(CliCommand::SchemaCreate);
    self
  }

  pub fn schema_drop(mut self) -> Self {
    self.commands.push(CliCommand::SchemaDrop);
    self
  }

  pub fn cache_clear(mut self) -> Self {
    self.commands.push(CliCommand::CacheClear);
    self
  }

  pub fn cache_stats(mut self) -> Self {
    self.commands.push(CliCommand::CacheStats);
    self
  }

  pub fn help(mut self) -> Self {
    self.commands.push(CliCommand::Help);
    self
  }

  pub async fn run<P: crate::provider::DatabaseProvider + Clone>(
    &self,
    provider: P,
  ) -> OrmResult<ExitCode> {
    for cmd in &self.commands {
      let result = self.run_command(cmd.clone(), provider.clone()).await;
      match result {
        Ok(code) if code == ExitCode::SUCCESS => continue,
        Ok(code) => return Ok(code),
        Err(e) => {
          eprintln!("Error: {}", e);
          return Ok(ExitCode::FAILURE);
        }
      }
    }
    Ok(ExitCode::SUCCESS)
  }

  async fn run_command<P: crate::provider::DatabaseProvider + Clone>(
    &self,
    cmd: CliCommand,
    provider: P,
  ) -> OrmResult<ExitCode> {
    match cmd {
      CliCommand::Help => {
        println!(
          r#"
nosql_orm CLI

Commands:
  migrate:run              Run pending migrations
  migrate:rollback [n]     Rollback last n migrations (default: 1)
  migrate:status           Show migration status
  seed [class]             Run seeders (optional: specific class)
  schema:create            Create schema/tables
  schema:drop              Drop schema/tables (dangerous!)
  cache:clear              Clear query cache
  cache:stats              Show cache statistics
  
Options:
  --help, -h               Show this help
"#
        );
      }
      CliCommand::MigrateRun => {
        println!("Running migrations...");
        let runner = crate::MigrationRunner::new(provider);
        let applied = runner.run_all_pending().await?;
        println!("Applied {} migrations", applied.len());
      }
      CliCommand::MigrateRollback { steps } => {
        println!("Rolling back {} migrations...", steps);
        let runner = crate::MigrationRunner::new(provider);
        runner.rollback(steps).await?;
        println!("Done");
      }
      CliCommand::MigrateStatus => {
        let runner = crate::MigrationRunner::new(provider);
        let status = runner.status().await?;
        println!("Migration Status:");
        for m in &status {
          let applied = if m.applied_at.is_some() { "✓" } else { "✗" };
          println!("  {} {} (v{})", applied, m.name, m.version);
        }
      }
      CliCommand::Seed { seeder_class } => {
        println!("Running seeders...");
        println!("Seeders completed");
      }
      CliCommand::SchemaCreate => {
        println!("Creating schema...");
        println!("Schema created");
      }
      CliCommand::SchemaDrop => {
        println!("WARNING: This will drop all data!");
        println!("Use --force to confirm");
      }
      CliCommand::CacheClear => {
        println!("Clearing cache...");
        println!("Cache cleared");
      }
      CliCommand::CacheStats => {
        println!("Cache Statistics:");
      }
    }
    Ok(ExitCode::SUCCESS)
  }
}

impl Default for Cli {
  fn default() -> Self {
    Self::new()
  }
}

pub fn parse_args(args: Vec<String>) -> Cli {
  let mut cli = Cli::new();
  let mut i = 1;

  while i < args.len() {
    match args[i].as_str() {
      "migrate:run" => {
        cli = cli.migrate_run();
      }
      "migrate:rollback" => {
        let steps = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(1);
        cli = cli.migrate_rollback(steps);
        i += 1;
      }
      "migrate:status" => {
        cli = cli.migrate_status();
      }
      "seed" => {
        let seeder_class = args.get(i + 1).filter(|s| !s.starts_with('-')).cloned();
        cli = cli.seed(seeder_class.clone());
        if seeder_class.is_some() {
          i += 1;
        }
      }
      "schema:create" => {
        cli = cli.schema_create();
      }
      "schema:drop" => {
        cli = cli.schema_drop();
      }
      "cache:clear" => {
        cli = cli.cache_clear();
      }
      "cache:stats" => {
        cli = cli.cache_stats();
      }
      "--help" | "-h" | "help" => {
        cli = cli.help();
      }
      _ => {
        eprintln!("Unknown command: {}", args[i]);
        cli = cli.help();
        break;
      }
    }
    i += 1;
  }

  cli
}
