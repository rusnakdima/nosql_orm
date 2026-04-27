//! Shared utilities for SQL providers.

use base64::{engine::general_purpose, Engine as _};

/// Encode binary data to base64 string.
pub fn base64_encode(data: &[u8]) -> String {
  general_purpose::STANDARD.encode(data)
}
