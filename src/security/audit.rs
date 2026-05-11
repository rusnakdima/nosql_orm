use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::OrmResult;
use crate::query::Filter;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub timestamp: DateTime<Utc>,
    pub user_id: Option<String>,
    pub action: SecurityAuditAction,
    pub entity_type: String,
    pub entity_id: String,
    pub changes: Option<ChangeSet>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityAuditAction {
    Create,
    Read,
    Update,
    Delete,
    Login,
    Logout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeSet {
    pub before: Option<serde_json::Value>,
    pub after: Option<serde_json::Value>,
    pub modified_fields: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct AuditFilters {
    pub user_id: Option<String>,
    pub action: Option<SecurityAuditAction>,
    pub entity_type: Option<String>,
    pub entity_id: Option<String>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub ip_address: Option<String>,
}

pub struct AuditLogger<D: Clone> {
    collection: String,
    provider: D,
}

impl<D: Clone> AuditLogger<D> {
    pub fn new(provider: D, audit_collection: &str) -> Self {
        Self {
            collection: audit_collection.to_string(),
            provider,
        }
    }
}

impl<D: crate::provider::DatabaseProvider + Clone> AuditLogger<D> {
    pub async fn log(&self, entry: AuditEntry) -> OrmResult<()> {
        let doc = serde_json::to_value(entry)?;
        self.provider.insert(&self.collection, doc).await?;
        Ok(())
    }

    pub async fn query(&self, filters: AuditFilters) -> OrmResult<Vec<AuditEntry>> {
        let mut filter_parts = Vec::new();

        if let Some(user_id) = &filters.user_id {
            filter_parts.push(Filter::Eq(
                "user_id".to_string(),
                serde_json::json!(user_id),
            ));
        }
        if let Some(entity_type) = &filters.entity_type {
            filter_parts.push(Filter::Eq(
                "entity_type".to_string(),
                serde_json::json!(entity_type),
            ));
        }
        if let Some(entity_id) = &filters.entity_id {
            filter_parts.push(Filter::Eq(
                "entity_id".to_string(),
                serde_json::json!(entity_id),
            ));
        }
        if let Some(ip_address) = &filters.ip_address {
            filter_parts.push(Filter::Eq(
                "ip_address".to_string(),
                serde_json::json!(ip_address),
            ));
        }
        if let Some(from_date) = &filters.from_date {
            filter_parts.push(Filter::Gte(
                "timestamp".to_string(),
                serde_json::json!(from_date.to_rfc3339()),
            ));
        }
        if let Some(to_date) = &filters.to_date {
            filter_parts.push(Filter::Lte(
                "timestamp".to_string(),
                serde_json::json!(to_date.to_rfc3339()),
            ));
        }

        let filter = if filter_parts.is_empty() {
            None
        } else {
            Some(Filter::And(filter_parts))
        };

        let entries = self
            .provider
            .find_many(
                &self.collection,
                filter.as_ref(),
                None,
                None,
                Some("timestamp"),
                false,
            )
            .await?;

        let mut result = Vec::new();
        for entry_value in entries {
            if let Ok(entry) = serde_json::from_value::<AuditEntry>(entry_value.clone()) {
                if let Some(ref action_filter) = filters.action {
                    if std::mem::discriminant(&entry.action)
                        == std::mem::discriminant(action_filter)
                    {
                        result.push(entry);
                    }
                } else {
                    result.push(entry);
                }
            }
        }

        Ok(result)
    }

    pub async fn log_action(
        &self,
        action: SecurityAuditAction,
        entity_type: &str,
        entity_id: &str,
        user_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
        changes: Option<ChangeSet>,
    ) -> OrmResult<()> {
        let entry = AuditEntry {
            timestamp: Utc::now(),
            user_id,
            action,
            entity_type: entity_type.to_string(),
            entity_id: entity_id.to_string(),
            changes,
            ip_address,
            user_agent,
        };
        self.log(entry).await
    }
}