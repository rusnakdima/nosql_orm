mod builder;
mod filter;
mod hints;
mod index_recommender;
mod optimizer;
mod projection;
mod stats;
mod types;
mod zero_copy;

pub use builder::QueryBuilder;
pub use filter::{parse_field_filter, Filter};
pub use hints::{OptimizationHint, QueryAnalyzer, QueryHint};
pub use index_recommender::{IndexRecommendation, IndexRecommender, QueryStats};
pub use optimizer::{OptimizedQuery, QueryOptimizer};
pub use projection::Projection;
pub use stats::{CollectionStats, FilterPattern, QueryStatistics};
pub use types::{Cursor, OrderBy, PaginatedResult, SortDirection};
pub use zero_copy::{DirectDeserializer, LazyValue, TypedArrayDeserializer, ZeroCopyDeserialize};
