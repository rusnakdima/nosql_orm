pub mod accessors;
pub mod mutators;

mod inner;

pub use inner::{extract_id, Entity, EntityMeta, FrontendProjection};
pub(crate) use inner::{expand_value, flatten_value};

pub use accessors::{Accessors, AccessorsExecutor, CachedAccessor, ComputedField, EntityAccessors};

pub use mutators::{CastType, EntityMutators, MutatorDef, Mutators, MutatorsExecutor};
