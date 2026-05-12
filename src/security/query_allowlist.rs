use crate::error::OrmError;
use std::collections::HashSet;
use std::sync::RwLock;

use regex::Regex;

pub struct QueryAllowlist {
    allowed_tables: HashSet<String>,
    allowed_operations: HashSet<String>,
    blocked_patterns: Vec<Regex>,
    enabled: bool,
    state: RwLock<AllowlistState>,
}

struct AllowlistState {
    total_checked: u64,
    blocked_count: u64,
}

impl QueryAllowlist {
    pub fn new() -> Self {
        Self {
            allowed_tables: HashSet::new(),
            allowed_operations: HashSet::from_iter(
                ["SELECT", "INSERT", "UPDATE", "DELETE"]
                    .iter()
                    .map(|s| s.to_string()),
            ),
            blocked_patterns: Vec::new(),
            enabled: true,
            state: RwLock::new(AllowlistState {
                total_checked: 0,
                blocked_count: 0,
            }),
        }
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn allow_table(&mut self, table: &str) {
        self.allowed_tables.insert(table.to_string());
    }

    pub fn allow_operation(&mut self, operation: &str) {
        self.allowed_operations.insert(operation.to_string());
    }

    fn validate_regex_pattern(pattern: &str) -> Result<(), OrmError> {
        if pattern.len() > 500 {
            return Err(OrmError::Validation(
                "Regex pattern exceeds maximum length of 500 characters".to_string(),
            ));
        }

        if pattern.contains("(?=") || pattern.contains("(?!") || pattern.contains("(?<=") || pattern.contains("(?<!") {
            return Err(OrmError::Validation(
                "Regex pattern contains prohibited lookahead/lookbehind assertions".to_string(),
            ));
        }

        if pattern.contains("{1000") || pattern.contains("{2000") || pattern.contains("{5000") {
            return Err(OrmError::Validation(
                "Regex pattern contains potentially catastrophic quantifier".to_string(),
            ));
        }

        Ok(())
    }

    pub fn block_pattern(&mut self, pattern: &str) -> Result<(), regex::Error> {
        Self::validate_regex_pattern(pattern).map_err(|e| regex::Error::Syntax(e.to_string()))?;
        let re = Regex::new(pattern)?;
        self.blocked_patterns.push(re);
        Ok(())
    }

    pub fn is_allowed(&self, sql: &str) -> bool {
        if !self.enabled {
            return true;
        }

        {
            let mut state = match self.state.write() {
                Ok(s) => s,
                Err(e) => {
                    return false;
                }
            };
            state.total_checked += 1;
        }

        let sql_upper = sql.to_uppercase();

        for pattern in &self.blocked_patterns {
            if pattern.is_match(&sql_upper) {
                let mut state = match self.state.write() {
                    Ok(s) => s,
                    Err(e) => {
                        return false;
                    }
                };
                state.blocked_count += 1;
                return false;
            }
        }

        let mut parts = sql_upper.split_whitespace();
        if let Some(operation) = parts.next() {
            if !self.allowed_operations.contains(operation) {
                let mut state = match self.state.write() {
                    Ok(s) => s,
                    Err(e) => {
                        return false;
                    }
                };
                state.blocked_count += 1;
                return false;
            }
        }

        if !self.allowed_tables.is_empty() {
            let from_present = sql_upper.contains("FROM");
            let into_present = sql_upper.contains("INTO");
            let update_present = sql_upper.contains("UPDATE");

            if from_present {
                if let Some(from_idx) = sql_upper.find("FROM") {
                    let after_from = &sql_upper[from_idx..];
                    if let Some(end_idx) = after_from[5..]
                        .find(|c: char| !c.is_alphanumeric() && c != '_' && c != ' ')
                    {
                        let table_name = after_from[5..5 + end_idx].trim();
                        if !self.allowed_tables.contains(table_name) {
                            let mut state = match self.state.write() {
                                Ok(s) => s,
                                Err(e) => {
                                    return false;
                                }
                            };
                            state.blocked_count += 1;
                            return false;
                        }
                    }
                }
            }

            if into_present {
                if let Some(into_idx) = sql_upper.find("INTO") {
                    let after_into = &sql_upper[into_idx..];
                    if let Some(end_idx) =
                        after_into[5..].find(|c: char| !c.is_alphanumeric() && c != '_' && c != ' ')
                    {
                        let table_name = after_into[5..5 + end_idx].trim();
                        if !self.allowed_tables.contains(table_name) {
                            let mut state = match self.state.write() {
                                Ok(s) => s,
                                Err(e) => {
                                    return false;
                                }
                            };
                            state.blocked_count += 1;
                            return false;
                        }
                    }
                }
            }

            if update_present {
                if let Some(update_idx) = sql_upper.find("UPDATE") {
                    let after_update = &sql_upper[update_idx..];
                    if let Some(end_idx) =
                        after_update[7..].find(|c: char| !c.is_alphanumeric() && c != '_' && c != ' ')
                    {
                        let table_name = after_update[7..7 + end_idx].trim();
                        if !self.allowed_tables.contains(table_name) {
                            let mut state = match self.state.write() {
                                Ok(s) => s,
                                Err(e) => {
                                    return false;
                                }
                            };
                            state.blocked_count += 1;
                            return false;
                        }
                    }
                }
            }
        }

        true
    }

    pub fn get_stats(&self) -> (u64, u64) {
        let state = match self.state.read() {
            Ok(s) => s,
            Err(_) => return (0, 0),
        };
        (state.total_checked, state.blocked_count)
    }

    pub fn reset_stats(&self) {
        if let Ok(mut state) = self.state.write() {
            state.total_checked = 0;
            state.blocked_count = 0;
        }
    }
}

impl Default for QueryAllowlist {
    fn default() -> Self {
        Self::new()
    }
}