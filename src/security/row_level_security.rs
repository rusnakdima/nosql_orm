use crate::entity::Entity;
use crate::query::Filter;

#[derive(Debug, Clone)]
pub struct SecurityPolicy {
    pub name: String,
    pub entity_type: String,
    pub expression: Filter,
}

impl SecurityPolicy {
    pub fn new(
        name: impl Into<String>,
        entity_type: impl Into<String>,
        expression: Filter,
    ) -> Self {
        Self {
            name: name.into(),
            entity_type: entity_type.into(),
            expression,
        }
    }
}

pub struct RowLevelSecurity {
    policies: Vec<SecurityPolicy>,
}

impl RowLevelSecurity {
    pub fn new() -> Self {
        Self {
            policies: Vec::new(),
        }
    }

    pub fn add_policy(&mut self, policy: SecurityPolicy) {
        self.policies.push(policy);
    }

    pub fn apply(&self, entity_type: &str, filter: Option<Filter>) -> Filter {
        let applicable_policies: Vec<&SecurityPolicy> = self
            .policies
            .iter()
            .filter(|p| p.entity_type == entity_type)
            .collect();

        if applicable_policies.is_empty() {
            return filter.unwrap_or(Filter::And(Vec::new()));
        }

        let policy_filters: Vec<Filter> = applicable_policies
            .iter()
            .map(|p| p.expression.clone())
            .collect();

        let combined_policy = if policy_filters.len() == 1 {
            policy_filters.into_iter().next().unwrap()
        } else {
            Filter::And(policy_filters)
        };

        match filter {
            Some(existing_filter) => Filter::And(vec![existing_filter, combined_policy]),
            None => combined_policy,
        }
    }

    pub fn check_access<E: Entity>(&self, entity: &E) -> bool {
        let entity_type = E::table_name();

        let applicable_policies: Vec<&SecurityPolicy> = self
            .policies
            .iter()
            .filter(|p| p.entity_type == entity_type)
            .collect();

        if applicable_policies.is_empty() {
            return true;
        }

        for policy in applicable_policies {
            let entity_value = entity.to_value().unwrap_or(serde_json::Value::Null);
            if !policy.expression.matches(&entity_value) {
                return false;
            }
        }

        true
    }

    pub fn get_policies(&self, entity_type: &str) -> Vec<&SecurityPolicy> {
        self.policies
            .iter()
            .filter(|p| p.entity_type == entity_type)
            .collect()
    }

    pub fn remove_policy(&mut self, name: &str) -> bool {
        let original_len = self.policies.len();
        self.policies.retain(|p| p.name != name);
        self.policies.len() != original_len
    }

    pub fn clear_policies(&mut self) {
        self.policies.clear();
    }
}

impl Default for RowLevelSecurity {
    fn default() -> Self {
        Self::new()
    }
}