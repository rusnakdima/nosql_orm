//! SQL type definitions for nosql_orm.

use serde::{Deserialize, Serialize};
use std::fmt::Display;

/// SQL dialect enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SqlDialect {
  PostgreSQL,
  SQLite,
  MySQL,
}

impl SqlDialect {
  pub fn quote_identifier(&self, name: &str) -> String {
    match self {
      SqlDialect::PostgreSQL => format!("\"{}\"", name),
      SqlDialect::SQLite => format!("\"{}\"", name),
      SqlDialect::MySQL => format!("`{}`", name),
    }
  }

  pub fn parameter_placeholder(&self, index: usize) -> String {
    match self {
      SqlDialect::PostgreSQL => format!("${}", index + 1),
      SqlDialect::SQLite => "?".to_string(),
      SqlDialect::MySQL => "?".to_string(),
    }
  }

  pub fn supports_batch(&self) -> bool {
    matches!(self, SqlDialect::PostgreSQL | SqlDialect::SQLite)
  }

  pub fn supports_on_conflict(&self) -> bool {
    matches!(self, SqlDialect::PostgreSQL | SqlDialect::SQLite)
  }
}

impl Display for SqlDialect {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      SqlDialect::PostgreSQL => write!(f, "PostgreSQL"),
      SqlDialect::SQLite => write!(f, "SQLite"),
      SqlDialect::MySQL => write!(f, "MySQL"),
    }
  }
}

/// SQL column data types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SqlColumnType {
  Integer,
  BigInt,
  SmallInt,
  Float,
  Double,
  Decimal(u8, u8),
  Boolean,
  Char(u32),
  VarChar(u32),
  Text,
  MediumText,
  LongText,
  Date,
  DateTime,
  Timestamp,
  TimestampTz,
  Time,
  Binary,
  Blob,
  Json,
  JsonB,
  Uuid,
  Serial,
  BigSerial,
}

impl SqlColumnType {
  pub fn to_sql(&self, dialect: SqlDialect) -> String {
    match self {
      SqlColumnType::Integer => "INTEGER".to_string(),
      SqlColumnType::BigInt => "BIGINT".to_string(),
      SqlColumnType::SmallInt => "SMALLINT".to_string(),
      SqlColumnType::Float => "FLOAT".to_string(),
      SqlColumnType::Double => "DOUBLE".to_string(),
      SqlColumnType::Decimal(p, s) => format!("DECIMAL({}, {})", p, s),
      SqlColumnType::Boolean => "BOOLEAN".to_string(),
      SqlColumnType::Char(n) => format!("CHAR({})", n),
      SqlColumnType::VarChar(n) => format!("VARCHAR({})", n),
      SqlColumnType::Text => "TEXT".to_string(),
      SqlColumnType::MediumText => match dialect {
        SqlDialect::MySQL => "MEDIUMTEXT".to_string(),
        _ => "TEXT".to_string(),
      },
      SqlColumnType::LongText => match dialect {
        SqlDialect::MySQL => "LONGTEXT".to_string(),
        _ => "TEXT".to_string(),
      },
      SqlColumnType::Date => "DATE".to_string(),
      SqlColumnType::DateTime => match dialect {
        SqlDialect::MySQL => "DATETIME".to_string(),
        _ => "TIMESTAMP".to_string(),
      },
      SqlColumnType::Timestamp => "TIMESTAMP".to_string(),
      SqlColumnType::TimestampTz => "TIMESTAMPTZ".to_string(),
      SqlColumnType::Time => "TIME".to_string(),
      SqlColumnType::Binary => "BYTEA".to_string(),
      SqlColumnType::Blob => "BLOB".to_string(),
      SqlColumnType::Json => "JSON".to_string(),
      SqlColumnType::JsonB => "JSONB".to_string(),
      SqlColumnType::Uuid => "UUID".to_string(),
      SqlColumnType::Serial => "SERIAL".to_string(),
      SqlColumnType::BigSerial => "BIGSERIAL".to_string(),
    }
  }
}

impl Display for SqlColumnType {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      SqlColumnType::Decimal(p, s) => write!(f, "DECIMAL({}, {})", p, s),
      SqlColumnType::Char(n) => write!(f, "CHAR({})", n),
      SqlColumnType::VarChar(n) => write!(f, "VARCHAR({})", n),
      _ => write!(f, "{:?}", self),
    }
  }
}

/// SQL index types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SqlIndexType {
  BTree,
  Hash,
  GiST,
  GIN,
  BRIN,
  SpGist,
}

impl SqlIndexType {
  pub fn to_sql(&self) -> &'static str {
    match self {
      SqlIndexType::BTree => "BTREE",
      SqlIndexType::Hash => "HASH",
      SqlIndexType::GiST => "GIST",
      SqlIndexType::GIN => "GIN",
      SqlIndexType::BRIN => "BRIN",
      SqlIndexType::SpGist => "SPGIST",
    }
  }
}

/// Primary key definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqlPrimaryKey {
  pub columns: Vec<String>,
  pub auto_increment: bool,
}

impl SqlPrimaryKey {
  pub fn new(columns: Vec<String>) -> Self {
    Self {
      columns,
      auto_increment: true,
    }
  }

  pub fn non_auto(columns: Vec<String>) -> Self {
    Self {
      columns,
      auto_increment: false,
    }
  }
}

/// SQL column definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqlColumnDef {
  pub name: String,
  pub column_type: SqlColumnType,
  pub nullable: bool,
  pub unique: bool,
  pub default: Option<String>,
  pub primary_key: bool,
  pub references: Option<(String, String)>,
  pub check: Option<String>,
}

impl SqlColumnDef {
  pub fn new(name: impl Into<String>, column_type: SqlColumnType) -> Self {
    Self {
      name: name.into(),
      column_type,
      nullable: false,
      unique: false,
      default: None,
      primary_key: false,
      references: None,
      check: None,
    }
  }

  pub fn nullable(mut self) -> Self {
    self.nullable = true;
    self
  }

  pub fn unique(mut self) -> Self {
    self.unique = true;
    self
  }

  pub fn default(mut self, value: impl Into<String>) -> Self {
    self.default = Some(value.into());
    self
  }

  pub fn primary_key(mut self) -> Self {
    self.primary_key = true;
    self
  }

  pub fn references(mut self, table: impl Into<String>, column: impl Into<String>) -> Self {
    self.references = Some((table.into(), column.into()));
    self
  }

  pub fn to_sql(&self, dialect: SqlDialect) -> String {
    let mut parts = vec![];

    let name = dialect.quote_identifier(&self.name);
    parts.push(name);
    parts.push(self.column_type.to_sql(dialect));

    if self.primary_key
      && (self.column_type == SqlColumnType::Serial || self.column_type == SqlColumnType::BigSerial)
    {
      parts.push("PRIMARY KEY".to_string());
    }

    if !self.nullable && !self.primary_key {
      parts.push("NOT NULL".to_string());
    }

    if self.unique && !self.primary_key {
      parts.push("UNIQUE".to_string());
    }

    if let Some(default) = &self.default {
      parts.push(format!("DEFAULT {}", default));
    }

    if let Some((table, column)) = &self.references {
      parts.push(format!(
        "REFERENCES {}({})",
        dialect.quote_identifier(table),
        dialect.quote_identifier(column)
      ));
    }

    if let Some(check) = &self.check {
      parts.push(format!("CHECK ({})", check));
    }

    parts.join(" ")
  }
}

/// SQL index definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqlIndexDef {
  pub name: String,
  pub table: String,
  pub columns: Vec<String>,
  pub unique: bool,
  pub index_type: SqlIndexType,
  pub concurrently: bool,
  pub where_clause: Option<String>,
}

impl SqlIndexDef {
  pub fn new(name: impl Into<String>, table: impl Into<String>, columns: Vec<String>) -> Self {
    Self {
      name: name.into(),
      table: table.into(),
      columns,
      unique: false,
      index_type: SqlIndexType::BTree,
      concurrently: false,
      where_clause: None,
    }
  }

  pub fn unique(mut self) -> Self {
    self.unique = true;
    self
  }

  pub fn index_type(mut self, index_type: SqlIndexType) -> Self {
    self.index_type = index_type;
    self
  }

  pub fn concurrently(mut self) -> Self {
    self.concurrently = true;
    self
  }

  pub fn where_clause(mut self, clause: impl Into<String>) -> Self {
    self.where_clause = Some(clause.into());
    self
  }

  pub fn to_sql(&self, dialect: SqlDialect) -> String {
    let _index_type_str = self.index_type.to_sql();
    let unique_str = if self.unique { "UNIQUE " } else { "" };

    let columns = self
      .columns
      .iter()
      .map(|c| dialect.quote_identifier(c))
      .collect::<Vec<_>>()
      .join(", ");

    let name = dialect.quote_identifier(&self.name);
    let table = dialect.quote_identifier(&self.table);

    let mut sql = format!(
      "CREATE {}INDEX {} ON {} ({})",
      unique_str, name, table, columns
    );

    if self.concurrently && dialect == SqlDialect::PostgreSQL {
      sql.push_str(" CONCURRENTLY");
    }

    if let Some(where_clause) = &self.where_clause {
      sql.push_str(&format!(" WHERE {}", where_clause));
    }

    sql
  }
}

/// SQL table definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqlTableDef {
  pub name: String,
  pub columns: Vec<SqlColumnDef>,
  pub primary_key: Option<SqlPrimaryKey>,
  pub foreign_keys: Vec<SqlForeignKey>,
  pub checks: Vec<String>,
  pub if_not_exists: bool,
}

impl SqlTableDef {
  pub fn new(name: impl Into<String>) -> Self {
    Self {
      name: name.into(),
      columns: Vec::new(),
      primary_key: None,
      foreign_keys: Vec::new(),
      checks: Vec::new(),
      if_not_exists: false,
    }
  }

  pub fn add_column(mut self, column: SqlColumnDef) -> Self {
    self.columns.push(column);
    self
  }

  pub fn primary_key(mut self, columns: Vec<String>) -> Self {
    self.primary_key = Some(SqlPrimaryKey::new(columns));
    self
  }

  pub fn if_not_exists(mut self) -> Self {
    self.if_not_exists = true;
    self
  }

  pub fn to_sql(&self, dialect: SqlDialect) -> String {
    let name = dialect.quote_identifier(&self.name);

    let mut column_defs = Vec::new();
    for col in &self.columns {
      column_defs.push(col.to_sql(dialect));
    }

    if let Some(pk) = &self.primary_key {
      let cols = pk
        .columns
        .iter()
        .map(|c| dialect.quote_identifier(c))
        .collect::<Vec<_>>()
        .join(", ");
      column_defs.push(format!("PRIMARY KEY ({})", cols));
    }

    for fk in &self.foreign_keys {
      column_defs.push(fk.to_sql(dialect));
    }

    for check in &self.checks {
      column_defs.push(format!("CHECK ({})", check));
    }

    let columns_str = column_defs.join(",\n    ");
    let if_not_exists_str = if self.if_not_exists {
      "IF NOT EXISTS "
    } else {
      ""
    };

    format!(
      "CREATE TABLE {}{} ({})",
      if_not_exists_str, name, columns_str
    )
  }
}

/// SQL foreign key definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqlForeignKey {
  pub columns: Vec<String>,
  pub foreign_table: String,
  pub foreign_columns: Vec<String>,
  pub on_delete: Option<SqlOnDelete>,
  pub on_update: Option<SqlOnUpdate>,
}

impl SqlForeignKey {
  pub fn new(
    columns: Vec<String>,
    foreign_table: impl Into<String>,
    foreign_columns: Vec<String>,
  ) -> Self {
    Self {
      columns,
      foreign_table: foreign_table.into(),
      foreign_columns,
      on_delete: None,
      on_update: None,
    }
  }

  pub fn on_delete(mut self, action: SqlOnDelete) -> Self {
    self.on_delete = Some(action);
    self
  }

  pub fn on_update(mut self, action: SqlOnUpdate) -> Self {
    self.on_update = Some(action);
    self
  }

  pub fn to_sql(&self, dialect: SqlDialect) -> String {
    let columns = self
      .columns
      .iter()
      .map(|c| dialect.quote_identifier(c))
      .collect::<Vec<_>>()
      .join(", ");

    let foreign_columns = self
      .foreign_columns
      .iter()
      .map(|c| dialect.quote_identifier(c))
      .collect::<Vec<_>>()
      .join(", ");

    let mut sql = format!(
      "FOREIGN KEY ({}) REFERENCES {}({})",
      columns,
      dialect.quote_identifier(&self.foreign_table),
      foreign_columns
    );

    if let Some(on_delete) = &self.on_delete {
      sql.push_str(&format!(" ON DELETE {}", on_delete));
    }

    if let Some(on_update) = &self.on_update {
      sql.push_str(&format!(" ON UPDATE {}", on_update));
    }

    sql
  }
}

/// ON DELETE action for foreign keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SqlOnDelete {
  Cascade,
  SetNull,
  SetDefault,
  Restrict,
  NoAction,
}

impl Display for SqlOnDelete {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      SqlOnDelete::Cascade => write!(f, "CASCADE"),
      SqlOnDelete::SetNull => write!(f, "SET NULL"),
      SqlOnDelete::SetDefault => write!(f, "SET DEFAULT"),
      SqlOnDelete::Restrict => write!(f, "RESTRICT"),
      SqlOnDelete::NoAction => write!(f, "NO ACTION"),
    }
  }
}

/// ON UPDATE action for foreign keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SqlOnUpdate {
  Cascade,
  SetNull,
  SetDefault,
  Restrict,
  NoAction,
}

impl Display for SqlOnUpdate {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      SqlOnUpdate::Cascade => write!(f, "CASCADE"),
      SqlOnUpdate::SetNull => write!(f, "SET NULL"),
      SqlOnUpdate::SetDefault => write!(f, "SET DEFAULT"),
      SqlOnUpdate::Restrict => write!(f, "RESTRICT"),
      SqlOnUpdate::NoAction => write!(f, "NO ACTION"),
    }
  }
}
