#[derive(Debug, Clone)]
pub struct GraphQLField {
  pub name: String,
  pub type_name: String,
  pub args: Vec<GraphQLArg>,
  pub resolver: Option<String>,
}

impl GraphQLField {
  pub fn new(name: &str, type_name: &str) -> Self {
    Self {
      name: name.to_string(),
      type_name: type_name.to_string(),
      args: Vec::new(),
      resolver: None,
    }
  }

  pub fn arg(mut self, name: &str, type_name: &str) -> Self {
    self.args.push(GraphQLArg {
      name: name.to_string(),
      type_name: type_name.to_string(),
    });
    self
  }

  pub fn resolver(mut self, resolver: &str) -> Self {
    self.resolver = Some(resolver.to_string());
    self
  }
}

#[derive(Debug, Clone)]
pub struct GraphQLArg {
  pub name: String,
  pub type_name: String,
}

#[derive(Debug, Clone)]
pub struct GraphQLTypeDef {
  pub name: String,
  pub fields: Vec<GraphQLField>,
}

impl GraphQLTypeDef {
  pub fn new(name: &str) -> Self {
    Self {
      name: name.to_string(),
      fields: Vec::new(),
    }
  }

  pub fn field(mut self, name: &str, type_name: &str) -> Self {
    self.fields.push(GraphQLField::new(name, type_name));
    self
  }
}

#[derive(Debug, Clone)]
pub struct SchemaBuilder {
  query_fields: Vec<GraphQLField>,
  mutation_fields: Vec<GraphQLField>,
  types: Vec<GraphQLTypeDef>,
}

impl SchemaBuilder {
  pub fn new() -> Self {
    Self {
      query_fields: Vec::new(),
      mutation_fields: Vec::new(),
      types: Vec::new(),
    }
  }

  pub fn add_query_field(mut self, field: GraphQLField) -> Self {
    self.query_fields.push(field);
    self
  }

  pub fn add_mutation_field(mut self, field: GraphQLField) -> Self {
    self.mutation_fields.push(field);
    self
  }

  pub fn add_type(mut self, type_def: GraphQLTypeDef) -> Self {
    self.types.push(type_def);
    self
  }

  pub fn generate_schema(&self) -> String {
    let mut schema = String::from("type Query {\n");
    for field in &self.query_fields {
      schema.push_str(&format!("  {}: {}\n", field.name, field.type_name));
    }
    schema.push_str("}\n\n");

    if !self.mutation_fields.is_empty() {
      schema.push_str("type Mutation {\n");
      for field in &self.mutation_fields {
        schema.push_str(&format!("  {}: {}\n", field.name, field.type_name));
      }
      schema.push_str("}\n");
    }

    for type_def in &self.types {
      schema.push_str(&format!("\ntype {} {{\n", type_def.name));
      for field in &type_def.fields {
        schema.push_str(&format!("  {}: {}\n", field.name, field.type_name));
      }
      schema.push_str("}\n");
    }

    schema
  }
}

impl Default for SchemaBuilder {
  fn default() -> Self {
    Self::new()
  }
}

pub trait GraphQLSchema {
  fn schema() -> SchemaBuilder;
}

#[derive(Debug, Clone)]
pub struct QueryRoot;

#[derive(Debug, Clone)]
pub struct MutationRoot;

pub trait GraphQLEntity {
  fn to_graphql_type() -> GraphQLTypeDef;
  fn to_graphql_input() -> GraphQLTypeDef;
}

impl GraphQLEntity for serde_json::Value {
  fn to_graphql_type() -> GraphQLTypeDef {
    let def = GraphQLTypeDef::new("JSON");
    def.field("value", "String")
  }

  fn to_graphql_input() -> GraphQLTypeDef {
    GraphQLTypeDef::new("JSONInput")
  }
}
