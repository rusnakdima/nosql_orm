use crate::relations::RelationType;
use crate::sql::types::SqlOnDelete;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum FieldType {
    Id,
    Column,
    Relation(RelationFieldType),
}

#[derive(Debug, Clone)]
pub enum RelationFieldType {
    OneToOne,
    OneToMany,
    ManyToOne,
    ManyToMany,
    ManyToOneArray,
}

#[derive(Debug, Clone)]
pub struct RelationMeta {
    pub name: String,
    pub relation_type: RelationType,
    pub target_entity: String,
    pub target_collection: String,
    pub local_key: String,
    pub foreign_key: String,
    pub join_field: Option<String>,
    pub local_key_in_array: Option<String>,
    pub on_delete: Option<SqlOnDelete>,
    pub cascade_soft_delete: bool,
    pub cascade_hard_delete: bool,
}

#[derive(Debug, Clone)]
pub struct ValidateMeta {
    pub validator: ValidatorType,
    pub params: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidatorType {
    Email,
    NotEmpty,
    NonNull,
    Length,
    Pattern,
    Range,
    Uuid,
    Url,
    Min,
    Max,
    Required,
}

#[derive(Debug, Clone)]
pub struct FieldMeta {
    pub name: String,
    pub field_type: FieldType,
    pub relation: Option<RelationMeta>,
    pub validators: Vec<ValidateMeta>,
    pub is_optional: bool,
    pub is_timestamp: bool,
    pub is_soft_delete: bool,
}

impl FieldMeta {
    pub fn new(name: String, field_type: FieldType) -> Self {
        Self {
            name,
            field_type,
            relation: None,
            validators: Vec::new(),
            is_optional: false,
            is_timestamp: false,
            is_soft_delete: false,
        }
    }

    pub fn with_relation(mut self, relation: RelationMeta) -> Self {
        self.relation = Some(relation);
        self
    }

    pub fn with_validators(mut self, validators: Vec<ValidateMeta>) -> Self {
        self.validators = validators;
        self
    }

    pub fn optional(mut self) -> Self {
        self.is_optional = true;
        self
    }

    pub fn timestamp(mut self) -> Self {
        self.is_timestamp = true;
        self
    }

    pub fn soft_delete(mut self) -> Self {
        self.is_soft_delete = true;
        self
    }
}

#[derive(Debug, Clone)]
pub struct EntityFieldMeta {
    pub fields: Vec<FieldMeta>,
    pub id_field: Option<String>,
    pub timestamp_fields: Vec<String>,
    pub soft_delete_field: Option<String>,
}

impl EntityFieldMeta {
    pub fn new() -> Self {
        Self {
            fields: Vec::new(),
            id_field: None,
            timestamp_fields: Vec::new(),
            soft_delete_field: None,
        }
    }

    pub fn add_field(&mut self, field: FieldMeta) {
        if let FieldType::Id = &field.field_type {
            self.id_field = Some(field.name.clone());
        }
        if field.is_timestamp {
            self.timestamp_fields.push(field.name.clone());
        }
        if field.is_soft_delete {
            self.soft_delete_field = Some(field.name.clone());
        }
        self.fields.push(field);
    }

    pub fn relation_fields(&self) -> Vec<&RelationMeta> {
        self.fields
            .iter()
            .filter_map(|f| f.relation.as_ref())
            .collect()
    }

    pub fn get_field(&self, name: &str) -> Option<&FieldMeta> {
        self.fields.iter().find(|f| f.name == name)
    }
}

impl Default for EntityFieldMeta {
    fn default() -> Self {
        Self::new()
    }
}

pub trait EntityFields {
    fn field_meta() -> EntityFieldMeta;
}