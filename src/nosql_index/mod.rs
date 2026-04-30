//! NoSQL-specific index types and management.

pub mod definition;
pub mod manager;

pub use definition::{NosqlIndex, NosqlIndexInfo, NosqlIndexType};
pub use manager::IndexManager;
