# SQL Utilities

SQL-specific types and query building.

---

## SqlColumnDef

```rust
pub struct SqlColumnDef {
    pub name: String,
    pub column_type: SqlColumnType,
    pub primary_key: bool,
    pub unique: bool,
    pub nullable: bool,
    pub default: Option<String>,
    pub check: Option<String>,
    pub references: Option<(String, String)>,
}

impl SqlColumnDef {
    pub fn new(name: &str, column_type: SqlColumnType) -> Self;
    pub fn primary_key(mut self) -> Self;
    pub fn unique(mut self) -> Self;
    pub fn nullable(mut self) -> Self;
    pub fn default(mut self, value: &str) -> Self;
    pub fn check(mut self, condition: &str) -> Self;
    pub fn references(mut self, table: &str, column: &str) -> Self;
}
```

## SqlColumnType

```rust
pub enum SqlColumnType {
    Serial,
    BigSerial,
    Boolean,
    Integer,
    BigInteger,
    SmallInteger,
    Float,
    Double,
    Char(usize),
    VarChar(usize),
    Text,
    Date,
    Time,
    DateTime,
    Timestamp,
    Json,
    JsonB,
    Uuid,
    Array(Box<SqlColumnType>),
}
```

## SqlDialect

```rust
pub enum SqlDialect {
    PostgreSQL,
    MySQL,
    SQLite,
}
```

## SqlQueryBuilder

```rust
pub struct SqlQueryBuilder { ... }

impl SqlQueryBuilder {
    pub fn new(dialect: SqlDialect) -> Self;
    pub fn select(&mut self, columns: &[&str]) -> Self;
    pub fn from(&mut self, table: &str) -> Self;
    pub fn where_eq(&mut self, field: &str, value: &str) -> Self;
    pub fn insert(&mut self, table: &str, columns: &[&str]) -> Self;
    pub fn update(&mut self, table: &str, columns: &[&str]) -> Self;
    pub fn delete(&mut self, table: &str) -> Self;
    pub fn build(&self) -> String;
}
```

---

## Example

```rust
let query = SqlQueryBuilder::new(SqlDialect::PostgreSQL)
    .select(&["id", "name", "email"])
    .from("users")
    .where_eq("status", "'active'")
    .build();

// SELECT id, name, email FROM users WHERE status = 'active'
```