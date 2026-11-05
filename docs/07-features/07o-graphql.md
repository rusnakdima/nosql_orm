# GraphQL Integration

GraphQL type generation from entities.

Opt-in: compiled only with the `graphql` feature. No code path in `Storage`,
`Repository` or the providers calls this module yet.

---

## GraphQLSchema

```rust
pub struct GraphQLSchema {
    query: QueryRoot,
    mutation: MutationRoot,
}
```

## GraphQLEntity

```rust
pub trait GraphQLEntity: Entity {
    fn graphql_type() -> GraphQLTypeDef;
    fn graphql_resolvers() -> Vec<GraphQLResolver>;
}
```

## Usage

```rust
let schema = SchemaBuilder::new()
    .entity::<User>()
    .entity::<Post>()
    .build();

let query = schema.query();
```