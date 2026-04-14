use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ColumnConstraint {
  NotNull,
  Unique,
  PrimaryKey,
  ForeignKey {
    table: String,
    column: String,
    on_delete: Option<String>,
    on_update: Option<String>,
  },
  Check(String),
  Default(serde_json::Value),
  AutoIncrement,
  Generated {
    expression: String,
    stored: bool,
  },
}

#[derive(Debug, Clone)]
pub struct ColumnDef {
  pub name: String,
  pub column_type: ColumnType,
  pub constraints: Vec<ColumnConstraint>,
  pub comment: Option<String>,
}

impl ColumnDef {
  pub fn new(name: &str, column_type: ColumnType) -> Self {
    Self {
      name: name.to_string(),
      column_type,
      constraints: Vec::new(),
      comment: None,
    }
  }

  pub fn not_null(mut self) -> Self {
    self.constraints.push(ColumnConstraint::NotNull);
    self
  }

  pub fn unique(mut self) -> Self {
    self.constraints.push(ColumnConstraint::Unique);
    self
  }

  pub fn primary_key(mut self) -> Self {
    self.constraints.push(ColumnConstraint::PrimaryKey);
    self
  }

  pub fn default(mut self, value: serde_json::Value) -> Self {
    self.constraints.push(ColumnConstraint::Default(value));
    self
  }

  pub fn auto_increment(mut self) -> Self {
    self.constraints.push(ColumnConstraint::AutoIncrement);
    self
  }

  pub fn foreign_key(mut self, table: &str, column: &str) -> Self {
    self.constraints.push(ColumnConstraint::ForeignKey {
      table: table.to_string(),
      column: column.to_string(),
      on_delete: None,
      on_update: None,
    });
    self
  }

  pub fn check(mut self, expression: &str) -> Self {
    self
      .constraints
      .push(ColumnConstraint::Check(expression.to_string()));
    self
  }

  pub fn comment(mut self, comment: &str) -> Self {
    self.comment = Some(comment.to_string());
    self
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColumnType {
  Integer,
  BigInt,
  SmallInt,
  Float,
  Double,
  Decimal,
  Boolean,
  Varchar,
  Text,
  Char,
  Date,
  Time,
  DateTime,
  Timestamp,
  TimestampTz,
  Blob,
  Binary,
  Json,
  JsonB,
  Uuid,
  Custom(String),
}

impl ColumnType {
  pub fn as_sql(&self, provider: &str) -> String {
    match self {
      ColumnType::Integer => "INTEGER".to_string(),
      ColumnType::BigInt => "BIGINT".to_string(),
      ColumnType::SmallInt => "SMALLINT".to_string(),
      ColumnType::Float => "FLOAT".to_string(),
      ColumnType::Double => "DOUBLE".to_string(),
      ColumnType::Decimal => "DECIMAL".to_string(),
      ColumnType::Boolean => "BOOLEAN".to_string(),
      ColumnType::Varchar => "VARCHAR".to_string(),
      ColumnType::Text => "TEXT".to_string(),
      ColumnType::Char => "CHAR".to_string(),
      ColumnType::Date => "DATE".to_string(),
      ColumnType::Time => "TIME".to_string(),
      ColumnType::DateTime => "DATETIME".to_string(),
      ColumnType::Timestamp => "TIMESTAMP".to_string(),
      ColumnType::TimestampTz => "TIMESTAMPTZ".to_string(),
      ColumnType::Blob => "BLOB".to_string(),
      ColumnType::Binary => "BINARY".to_string(),
      ColumnType::Json => if provider == "postgres" {
        "JSONB"
      } else {
        "JSON"
      }
      .to_string(),
      ColumnType::JsonB => "JSONB".to_string(),
      ColumnType::Uuid => if provider == "postgres" {
        "UUID"
      } else {
        "TEXT"
      }
      .to_string(),
      ColumnType::Custom(s) => s.clone(),
    }
  }
}

#[derive(Debug, Clone)]
pub struct UniqueConstraintDef {
  pub columns: Vec<String>,
  pub name: Option<String>,
}

impl UniqueConstraintDef {
  pub fn new(columns: Vec<String>) -> Self {
    Self {
      columns,
      name: None,
    }
  }

  pub fn name(mut self, name: &str) -> Self {
    self.name = Some(name.to_string());
    self
  }
}

#[derive(Debug, Clone)]
pub struct CheckConstraintDef {
  pub name: Option<String>,
  pub expression: String,
}

impl CheckConstraintDef {
  pub fn new(expression: &str) -> Self {
    Self {
      name: None,
      expression: expression.to_string(),
    }
  }

  pub fn name(mut self, name: &str) -> Self {
    self.name = Some(name.to_string());
    self
  }
}
