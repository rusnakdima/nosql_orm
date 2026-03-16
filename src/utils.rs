use uuid::Uuid;

/// Generate a new random UUIDv4 string.
pub fn generate_id() -> String {
  Uuid::new_v4().to_string()
}

/// Generate a short 8-character id suitable for display.
pub fn short_id() -> String {
  Uuid::new_v4().to_string().replace('-', "")[..8].to_string()
}
