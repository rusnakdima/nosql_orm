// Versioned, machine-readable structure manifest for JSON collections.
//
// This module is the single source of truth for the "3NF-style" contract the
// engine checks over canonical JSON / MongoDB collections. It is kept in the
// data layer (`nosql_orm`) so that both the engine validator (`forge::verify`)
// and the JSON/Mongo providers can use the exact same types and the exact same
// `validate_records` core — that is what makes the two providers consistent.
//
// # Manifest shape (canonical file: `schemas_db/forge/schema/collections.json`)
//
// ```json
// {
//   "schema_version": "1.0.0",
//   "kind": "forge.structure",
//   "mode": "report",
//   "collections": {
//     "tasks": {
//       "source": "tasks.json",
//       "primary_key": ["id"],
//       "fields": {
//         "id":            {"type": "string", "required": true, "unique": true},
//         "project_id":    {"type": "relation", "required": true, "ref": "projects.id"},
//         "status":        {"type": "string", "required": true, "enum": "TaskStatus"},
//         "subtasks_count":{"type": "int", "derived": "count(subtasks.task_id)"}
//       },
//       "unique": [{"name": "uq_tasks_title_per_project", "fields": ["project_id","title"]}],
//       "owned_children": [{"collection": "subtasks", "via": "task_id", "parent_field": "subtasks"}],
//       "forbidden_repeating_groups": ["assignees[]"],
//       "replacement_collections": ["project_assignees"]
//     }
//   },
//   "enums": {"TaskStatus": ["pending","completed","skipped","failed"]},
//   "indexes": [{"collection":"tasks","name":"idx_tasks_project","fields":["project_id"],"unique":false}]
// }
// ```
//
// Field semantics:
// * `type` — `string | int | number | bool | date | json | array | object |
//   relation`; a `[]` suffix (e.g. `relation[]`) means an array whose every
//   element matches the base type. Unknown type names fail open (advisory).
// * `required` — field must be present and non-null in every record.
// * `unique` — no two records may carry the same non-null value.
// * `ref` — foreign key, written `collection.field`; works for scalar and
//   array-valued fields (every element must resolve).
// * `enum` — name of a lookup array in the top-level `enums` map.
// * `derived` — `count(child_collection.child_field)` marks a denormalized
//   cache: a `transitive_dependency` warning is always reported, and the
//   cached value is compared with the recomputed count (`derived_stale`).
// * `on_delete` / `replacement_collections` are advisory metadata for
//   authors and tooling; the validator does not act on them.
//
// Collection semantics:
// * `primary_key` — compound PK allowed; an empty list means "use `id` if
//   present, otherwise no PK check" (backward compatible with old content).
// * `unique` — named compound unique constraints.
// * `owned_children` — the LEGITIMATE 1:N case: child records live in their own
//   collection keyed by `via` (a `ref` back to this collection). `parent_field`
//   names an optional array of child PKs on the parent; the same child PK
//   appearing under two parents is `duplicated_owned_child`.
// * `forbidden_repeating_groups` — fields that must NOT be arrays/objects of
//   embedded duplicates (the SQL repeating-group anti-pattern). The `[]`
//   suffix is optional when declaring them.
//
// `mode` is `report` by default: violations are structured data, reported by
// the caller, never fatal. `strict` escalates every warning to Error. Provider
// write paths always reject Error-severity violations; collections with no
// manifest entry keep their exact prior behavior.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::path::Path;

use crate::nosql_index::NosqlIndex;

// Wire format version. One family only; a different major version is an error.
pub const STRUCTURE_SCHEMA_VERSION: &str = "1.0.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum StructureMode {
  #[default]
  Report,
  Strict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StructureSeverity {
  Error,
  Warning,
}

impl StructureSeverity {
  pub fn as_str(self) -> &'static str {
    match self {
      Self::Error => "error",
      Self::Warning => "warning",
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StructureViolationCode {
  DuplicatePrimaryKey,
  MissingRequired,
  BrokenForeignKey,
  UniqueViolation,
  TypeMismatch,
  EnumViolation,
  ForbiddenRepeatingGroup,
  DuplicatedOwnedChild,
  TransitiveDependency,
  DerivedStale,
}

impl StructureViolationCode {
  pub fn as_str(self) -> &'static str {
    match self {
      Self::DuplicatePrimaryKey => "duplicate_pk",
      Self::MissingRequired => "missing_required",
      Self::BrokenForeignKey => "broken_fk",
      Self::UniqueViolation => "unique_violation",
      Self::TypeMismatch => "type_mismatch",
      Self::EnumViolation => "enum_violation",
      Self::ForbiddenRepeatingGroup => "forbidden_repeating_group",
      Self::DuplicatedOwnedChild => "duplicated_owned_child",
      Self::TransitiveDependency => "transitive_dependency",
      Self::DerivedStale => "derived_stale",
    }
  }
}

// A structured, machine-readable validation finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructureViolation {
  pub code: StructureViolationCode,
  pub collection: String,
  // Primary-key value of the offending record, or `index:N` when it has none.
  pub record: String,
  pub field: Option<String>,
  pub message: String,
  pub severity: StructureSeverity,
}

impl fmt::Display for StructureViolation {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      f,
      "{}: {} [{}.{}] {}",
      self.severity.as_str(),
      self.code.as_str(),
      self.collection,
      self.record,
      self.message
    )
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructureError {
  Io(String),
  Parse(String),
  InvalidManifest(String),
  UnsupportedVersion(String),
}

impl fmt::Display for StructureError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Io(m) => write!(f, "structure manifest I/O error: {}", m),
      Self::Parse(m) => write!(f, "structure manifest parse error: {}", m),
      Self::InvalidManifest(m) => write!(f, "invalid structure manifest: {}", m),
      Self::UnsupportedVersion(m) => write!(f, "unsupported structure schema version: {}", m),
    }
  }
}

impl std::error::Error for StructureError {}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StructureManifest {
  #[serde(default = "default_schema_version")]
  pub schema_version: String,
  #[serde(default)]
  pub kind: Option<String>,
  #[serde(default)]
  pub mode: StructureMode,
  #[serde(default)]
  pub collections: BTreeMap<String, CollectionStructure>,
  #[serde(default)]
  pub enums: BTreeMap<String, Vec<String>>,
  #[serde(default)]
  pub indexes: Vec<IndexDeclaration>,
}

fn default_schema_version() -> String {
  STRUCTURE_SCHEMA_VERSION.to_string()
}

// Parse and validate a JSON manifest document.
pub fn parse_manifest(json: &str) -> Result<StructureManifest, StructureError> {
  StructureManifest::parse(json)
}

// Load and validate a manifest file from disk.
pub fn load_manifest(path: &Path) -> Result<StructureManifest, StructureError> {
  StructureManifest::load(path)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CollectionStructure {
  // Filename holding the record array, relative to the content root.
  #[serde(default)]
  pub source: Option<String>,
  #[serde(default)]
  pub primary_key: Vec<String>,
  #[serde(default)]
  pub fields: BTreeMap<String, FieldStructure>,
  #[serde(default)]
  pub unique: Vec<UniqueConstraint>,
  #[serde(default)]
  pub owned_children: Vec<OwnedChild>,
  #[serde(default)]
  pub forbidden_repeating_groups: Vec<String>,
  #[serde(default)]
  pub replacement_collections: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FieldStructure {
  #[serde(default, rename = "type")]
  pub field_type: Option<String>,
  #[serde(default)]
  pub required: bool,
  #[serde(default)]
  pub unique: bool,
  #[serde(default, rename = "enum")]
  pub enum_name: Option<String>,
  #[serde(default, rename = "ref")]
  pub reference: Option<String>,
  #[serde(default)]
  pub on_delete: Option<String>,
  #[serde(default)]
  pub derived: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UniqueConstraint {
  #[serde(default)]
  pub name: String,
  #[serde(default)]
  pub fields: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OwnedChild {
  pub collection: String,
  // Field on the child pointing back at this collection's PK.
  pub via: String,
  // Optional array field on the parent holding child PKs.
  #[serde(default)]
  pub parent_field: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndexDeclaration {
  pub collection: String,
  #[serde(default)]
  pub name: Option<String>,
  #[serde(default)]
  pub fields: Vec<String>,
  #[serde(default)]
  pub unique: bool,
}

impl StructureManifest {
  pub fn parse(json: &str) -> Result<Self, StructureError> {
    let manifest: Self =
      serde_json::from_str(json).map_err(|e| StructureError::Parse(e.to_string()))?;
    manifest.validate()?;
    Ok(manifest)
  }

  pub fn load(path: &Path) -> Result<Self, StructureError> {
    let text = std::fs::read_to_string(path)
      .map_err(|e| StructureError::Io(format!("{}: {}", path.display(), e)))?;
    Self::parse(&text)
  }

  // Version check and basic referential sanity of the manifest itself.
  pub fn validate(&self) -> Result<(), StructureError> {
    let parts: Vec<&str> = self.schema_version.split('.').collect();
    if parts.len() != 3
      || parts
        .iter()
        .any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()))
    {
      return Err(StructureError::UnsupportedVersion(
        self.schema_version.clone(),
      ));
    }
    let major: u32 = parts[0]
      .parse()
      .map_err(|_| StructureError::UnsupportedVersion(self.schema_version.clone()))?;
    if major != 1 {
      return Err(StructureError::UnsupportedVersion(
        self.schema_version.clone(),
      ));
    }
    for (name, collection) in &self.collections {
      for field in &collection.primary_key {
        if !collection.fields.contains_key(field) {
          return Err(StructureError::InvalidManifest(format!(
            "{}.primary_key names undeclared field '{}'",
            name, field
          )));
        }
      }
      for constraint in &collection.unique {
        if constraint.fields.is_empty() {
          return Err(StructureError::InvalidManifest(format!(
            "{}.unique '{}' has no fields",
            name, constraint.name
          )));
        }
      }
      for child in &collection.owned_children {
        if !self.collections.contains_key(&child.collection) {
          return Err(StructureError::InvalidManifest(format!(
            "{}.owned_children names unknown collection '{}'",
            name, child.collection
          )));
        }
      }
    }
    for index in &self.indexes {
      if !self.collections.contains_key(&index.collection) {
        return Err(StructureError::InvalidManifest(format!(
          "index names unknown collection '{}'",
          index.collection
        )));
      }
    }
    Ok(())
  }

  pub fn collection(&self, name: &str) -> Option<&CollectionStructure> {
    self.collections.get(name)
  }

  // Field sets this manifest declares unique for a collection, including the
  // primary key and every named compound constraint. Used by provider writes.
  pub fn unique_field_sets(&self, collection: &str) -> Vec<Vec<String>> {
    let Some(cs) = self.collections.get(collection) else {
      return Vec::new();
    };
    let mut sets: Vec<Vec<String>> = Vec::new();
    if !cs.primary_key.is_empty() {
      sets.push(cs.primary_key.clone());
    }
    for (field, fs) in &cs.fields {
      if fs.unique && !sets.iter().any(|s| s.len() == 1 && s[0] == *field) {
        sets.push(vec![field.clone()]);
      }
    }
    for uc in &cs.unique {
      if !sets.contains(&uc.fields) {
        sets.push(uc.fields.clone());
      }
    }
    sets
  }

  // Convert manifest index declarations + unique constraints + PKs into the
  // provider-neutral `NosqlIndex` format used by both JSON and Mongo.
  pub fn index_declarations(&self) -> Vec<(String, NosqlIndex)> {
    let mut out = Vec::new();
    for index in &self.indexes {
      let fields: Vec<(&str, i32)> = index.fields.iter().map(|f| (f.as_str(), 1i32)).collect();
      let mut built = if fields.len() == 1 {
        NosqlIndex::single(fields[0].0, 1)
      } else {
        NosqlIndex::compound(&fields)
      };
      if let Some(name) = &index.name {
        built = built.name(name);
      }
      if index.unique {
        built = built.unique();
      }
      out.push((index.collection.clone(), built));
    }
    for (name, cs) in &self.collections {
      if !cs.primary_key.is_empty() {
        let fields: Vec<(&str, i32)> = cs.primary_key.iter().map(|f| (f.as_str(), 1i32)).collect();
        let built = if fields.len() == 1 {
          NosqlIndex::single(fields[0].0, 1)
        } else {
          NosqlIndex::compound(&fields)
        };
        out.push((name.clone(), built.unique().name(&format!("pk_{}", name))));
      }
      for uc in &cs.unique {
        let fields: Vec<(&str, i32)> = uc.fields.iter().map(|f| (f.as_str(), 1i32)).collect();
        let built = NosqlIndex::compound(&fields).unique().name(&uc.name);
        out.push((name.clone(), built));
      }
      for (field, fs) in &cs.fields {
        if fs.unique {
          out.push((
            name.clone(),
            NosqlIndex::single(field, 1)
              .unique()
              .name(&format!("uq_{}_{}", name, field)),
          ));
        }
      }
    }
    out
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// Validation core
// ─────────────────────────────────────────────────────────────────────────────

fn violation(
  code: StructureViolationCode,
  collection: &str,
  record: &str,
  field: Option<&str>,
  message: impl Into<String>,
) -> StructureViolation {
  StructureViolation {
    code,
    collection: collection.to_string(),
    record: record.to_string(),
    field: field.map(str::to_string),
    message: message.into(),
    severity: match code {
      StructureViolationCode::TransitiveDependency => StructureSeverity::Warning,
      _ => StructureSeverity::Error,
    },
  }
}

fn pk_component(value: &Value) -> Option<String> {
  match value {
    Value::String(s) => Some(s.clone()),
    Value::Number(n) => Some(n.to_string()),
    Value::Bool(b) => Some(b.to_string()),
    _ => None,
  }
}

fn record_label(record: &Value, pk: &[String], index: usize) -> String {
  if pk.len() == 1 {
    if let Some(s) = record.get(&pk[0]).and_then(pk_component) {
      return s;
    }
  }
  format!("index:{}", index)
}

fn type_matches(declared: &str, value: &Value) -> bool {
  let declared = declared.trim();
  let (base, array) = match declared.strip_suffix("[]") {
    Some(rest) => (rest.trim(), true),
    None => (declared, false),
  };
  if array {
    return value
      .as_array()
      .is_some_and(|items| items.iter().all(|item| type_matches(base, item)));
  }
  match base {
    "string" | "str" | "date" | "datetime" | "relation" | "id" | "enum" => value.is_string(),
    "int" | "integer" => value.is_i64() || value.is_u64(),
    "number" | "float" | "decimal" => value.is_number(),
    "bool" | "boolean" => value.is_boolean(),
    "array" | "list" => value.is_array(),
    "object" | "map" => value.is_object(),
    // `json` (and unknown names) are advisory: never reject existing content.
    _ => true,
  }
}

fn ref_parts(reference: &str) -> Option<(&str, &str)> {
  reference.split_once('.')
}

fn check_fk(
  collections: &BTreeMap<String, Vec<Value>>,
  reference: &str,
  value: &Value,
  collection: &str,
  record: &str,
  field: &str,
  out: &mut Vec<StructureViolation>,
) {
  let Some((target_collection, target_field)) = ref_parts(reference) else {
    out.push(violation(
      StructureViolationCode::BrokenForeignKey,
      collection,
      record,
      Some(field),
      format!("malformed ref '{}' (expected collection.field)", reference),
    ));
    return;
  };
  // A collection absent from the snapshot cannot be verified — fail open.
  let Some(targets) = collections.get(target_collection) else {
    return;
  };
  let candidates: Vec<&Value> = if let Some(array) = value.as_array() {
    array.iter().collect()
  } else {
    vec![value]
  };
  for candidate in candidates {
    let Some(key) = pk_component(candidate) else {
      continue;
    };
    if !targets.iter().any(|doc| {
      doc
        .get(target_field)
        .and_then(pk_component)
        .is_some_and(|found| found == key)
    }) {
      out.push(violation(
        StructureViolationCode::BrokenForeignKey,
        collection,
        record,
        Some(field),
        format!(
          "'{}' does not resolve in {}.{}",
          key, target_collection, target_field
        ),
      ));
    }
  }
}

fn check_unique(
  records: &[Value],
  fields: &[String],
  pk: &[String],
  collection: &str,
  label: Option<&str>,
  out: &mut Vec<StructureViolation>,
) {
  let mut seen: HashMap<Vec<String>, String> = HashMap::new();
  for (index, record) in records.iter().enumerate() {
    let mut key = Vec::with_capacity(fields.len());
    let mut complete = true;
    for field in fields {
      match record
        .get(field)
        .and_then(|v| if v.is_null() { None } else { pk_component(v) })
      {
        Some(part) => key.push(part),
        None => {
          complete = false;
          break;
        }
      }
    }
    if !complete {
      continue;
    }
    let label = label
      .map(str::to_string)
      .unwrap_or_else(|| record_label(record, pk, index));
    if let Some(previous) = seen.insert(key.clone(), label.clone()) {
      out.push(violation(
        StructureViolationCode::UniqueViolation,
        collection,
        &label,
        Some(&fields.join("+")),
        format!(
          "value {} duplicates record '{}' on unique constraint [{}]",
          key.join("|"),
          previous,
          fields.join(", ")
        ),
      ));
    }
  }
}

fn derived_parts(derived: &str) -> Option<(&str, &str)> {
  let inner = derived
    .strip_prefix("count(")
    .and_then(|rest| rest.strip_suffix(')'))?;
  ref_parts(inner.trim())
}

// Validate every declared collection present in `collections`.
//
// Pure: no I/O, no provider calls. Both `forge::verify` and the JSON/Mongo
// providers call this exact function, which is what guarantees consistency.
pub fn validate_records(
  manifest: &StructureManifest,
  collections: &BTreeMap<String, Vec<Value>>,
) -> Vec<StructureViolation> {
  let mut out = Vec::new();

  for (name, structure) in &manifest.collections {
    let Some(records) = collections.get(name) else {
      continue; // collection not shipped — nothing to check
    };
    let pk = &structure.primary_key;

    // A derived field is a 3NF smell regardless of whether its cached value is
    // correct. Report it once per declaration, not once per record.
    for (field, fs) in &structure.fields {
      if fs.derived.is_some() {
        out.push(violation(
          StructureViolationCode::TransitiveDependency,
          name,
          "<collection>",
          Some(field),
          format!(
            "field '{}' is a denormalized cache ('{}'); 3NF would recompute it",
            field,
            fs.derived.as_deref().unwrap_or("")
          ),
        ));
      }
    }

    let mut seen_pk: HashMap<Vec<String>, String> = HashMap::new();
    let mut child_owners: HashMap<(String, String), String> = HashMap::new();

    for (index, record) in records.iter().enumerate() {
      let label = record_label(record, pk, index);

      // Primary key: presence, then duplicate detection.
      if !pk.is_empty() {
        let mut key = Vec::with_capacity(pk.len());
        let mut missing: Option<&str> = None;
        for field in pk {
          match record
            .get(field)
            .and_then(|v| if v.is_null() { None } else { pk_component(v) })
          {
            Some(part) => key.push(part),
            None => {
              missing = Some(field);
              break;
            }
          }
        }
        if let Some(field) = missing {
          out.push(violation(
            StructureViolationCode::MissingRequired,
            name,
            &label,
            Some(field),
            "primary key field is missing or null".to_string(),
          ));
        } else if let Some(previous) = seen_pk.insert(key.clone(), label.clone()) {
          out.push(violation(
            StructureViolationCode::DuplicatePrimaryKey,
            name,
            &label,
            Some(&pk.join("+")),
            format!(
              "primary key [{}] = {} duplicates record '{}'",
              pk.join(", "),
              key.join("|"),
              previous
            ),
          ));
        }
      }

      // Field-level checks.
      for (field, fs) in &structure.fields {
        let value = record.get(field).filter(|v| !v.is_null());
        match value {
          None => {
            if fs.required || (!pk.is_empty() && pk.contains(field)) {
              // PK-missing is already reported above; avoid the double report.
              if !(!pk.is_empty() && pk.contains(field)) {
                out.push(violation(
                  StructureViolationCode::MissingRequired,
                  name,
                  &label,
                  Some(field),
                  "required field is missing or null".to_string(),
                ));
              }
            }
          }
          Some(value) => {
            if let Some(declared) = &fs.field_type {
              if !type_matches(declared, value) {
                out.push(violation(
                  StructureViolationCode::TypeMismatch,
                  name,
                  &label,
                  Some(field),
                  format!("expected type '{}'", declared),
                ));
              }
            }
            if let Some(enum_name) = &fs.enum_name {
              if let Some(allowed) = manifest.enums.get(enum_name) {
                let values: Vec<&Value> = if let Some(array) = value.as_array() {
                  array.iter().collect()
                } else {
                  vec![value]
                };
                for item in values {
                  if !item.is_string() || !allowed.iter().any(|v| v == item) {
                    out.push(violation(
                      StructureViolationCode::EnumViolation,
                      name,
                      &label,
                      Some(field),
                      format!("value {} is not in enum '{}'", item, enum_name),
                    ));
                  }
                }
              }
            }
            if let Some(reference) = &fs.reference {
              check_fk(collections, reference, value, name, &label, field, &mut out);
            }
            if let Some(derived) = &fs.derived {
              if let Some((child_collection, child_field)) = derived_parts(derived) {
                if let Some(children) = collections.get(child_collection) {
                  let parent_key = if pk.len() == 1 {
                    record.get(&pk[0]).and_then(pk_component)
                  } else {
                    None
                  };
                  if let Some(parent_key) = parent_key {
                    let actual = children
                      .iter()
                      .filter(|child| {
                        child
                          .get(child_field)
                          .and_then(pk_component)
                          .is_some_and(|v| v == parent_key)
                      })
                      .count() as f64;
                    let cached = value.as_f64();
                    if cached.is_some_and(|cached| (cached - actual).abs() > f64::EPSILON) {
                      out.push(violation(
                        StructureViolationCode::DerivedStale,
                        name,
                        &label,
                        Some(field),
                        format!(
                          "cached value {} but {}.{} has {} matching records",
                          value, child_collection, child_field, actual
                        ),
                      ));
                    }
                  }
                }
              }
            }
          }
        }
      }

      // Repeating groups stored inside the parent record.
      for forbidden in &structure.forbidden_repeating_groups {
        let field = forbidden.strip_suffix("[]").unwrap_or(forbidden);
        if let Some(value) = record.get(field) {
          let non_empty = value.as_array().is_some_and(|array| !array.is_empty())
            || value.as_object().is_some_and(|map| !map.is_empty());
          if non_empty {
            out.push(violation(
              StructureViolationCode::ForbiddenRepeatingGroup,
              name,
              &label,
              Some(field),
              format!(
                "'{}' embeds a repeating group; use a child collection ({})",
                field,
                if structure.replacement_collections.is_empty() {
                  "see manifest".to_string()
                } else {
                  structure.replacement_collections.join(", ")
                }
              ),
            ));
          }
        }
      }

      // Owned children: the same child must not be embedded under two parents.
      for child in &structure.owned_children {
        let Some(parent_field) = &child.parent_field else {
          continue;
        };
        let Some(array) = record.get(parent_field).and_then(Value::as_array) else {
          continue;
        };
        let child_pk = manifest
          .collections
          .get(&child.collection)
          .map(|c| c.primary_key.clone())
          .unwrap_or_else(|| vec!["id".to_string()]);
        for item in array {
          let key = if child_pk.len() == 1 {
            // Parent arrays may hold bare PK values ("c1") or child objects
            // ({"id":"c1"}); canonical content uses both shapes.
            if item.is_string() || item.is_number() {
              pk_component(item)
            } else {
              item.get(&child_pk[0]).and_then(pk_component)
            }
          } else {
            None
          };
          let Some(key) = key else { continue };
          let map_key = (child.collection.clone(), key.clone());
          if let Some(previous) = child_owners.get(&map_key) {
            if previous != &label {
              out.push(violation(
                StructureViolationCode::DuplicatedOwnedChild,
                name,
                &label,
                Some(parent_field),
                format!(
                  "child '{}' of '{}' is also embedded under parent '{}'",
                  key, child.collection, previous
                ),
              ));
            }
          } else {
            child_owners.insert(map_key, label.clone());
          }
        }
      }
    }

    // Collection-level unique constraints (after the record loop so that a
    // duplicate is always the *second* record we see).
    for (field, fs) in &structure.fields {
      if fs.unique {
        check_unique(
          records,
          std::slice::from_ref(field),
          pk,
          name,
          None,
          &mut out,
        );
      }
    }
    for constraint in &structure.unique {
      check_unique(records, &constraint.fields, pk, name, None, &mut out);
    }
  }

  if manifest.mode == StructureMode::Strict {
    for finding in &mut out {
      finding.severity = StructureSeverity::Error;
    }
  }

  out
}

// Convenience: only the Error-severity findings.
pub fn blocking_violations(findings: &[StructureViolation]) -> Vec<&StructureViolation> {
  findings
    .iter()
    .filter(|v| v.severity == StructureSeverity::Error)
    .collect()
}

// Convert findings into a deduplicated set of collection names.
pub fn affected_collections(findings: &[StructureViolation]) -> HashSet<String> {
  findings.iter().map(|v| v.collection.clone()).collect()
}

#[cfg(test)]
mod tests;
