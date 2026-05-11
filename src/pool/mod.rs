pub mod adaptive;
pub mod auto_scaling;
pub mod health;
pub mod lifecycle;
pub mod metrics;
pub mod pool_impl;
pub mod replica;

#[cfg(feature = "mongo")]
pub use pool_impl::MongoPool;

pub use adaptive::{AdaptivePool, AdaptivePoolProvider, AdaptivePooled};
pub use auto_scaling::{AutoScaler, AutoScalerConfig};
pub use health::{ConnectionHealthMonitor, HealthCheckable, HealthStatus};
pub use lifecycle::{ConnectionHealth, ConnectionLifecycle};
pub use metrics::PoolMetrics;
pub use pool_impl::{JsonPool, Pool, PoolConfig, Pooled};
pub use replica::{ReadSelection, ReplicaConfig, ReplicaPool};
