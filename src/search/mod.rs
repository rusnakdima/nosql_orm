pub mod fulltext;
pub mod search_impl;

pub use fulltext::{FullTextIndex, FullTextSearch};
pub use search_impl::{FullTextQueryExt, SearchResult, SearchScore, TextSearch};
