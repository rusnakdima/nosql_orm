// Relation loading — module root is a map (§17e).
mod cascade;
mod loader_impl;
mod many_to_many;
mod many_to_one;
mod nested;
mod one_to_many;
mod recursive;

pub use loader_impl::RelationLoader;

#[cfg(test)]
mod tests;
