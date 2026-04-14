#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphQLScalar {
  String,
  Int,
  Float,
  Boolean,
  ID,
}

impl GraphQLScalar {
  pub fn as_str(&self) -> &'static str {
    match self {
      GraphQLScalar::String => "String",
      GraphQLScalar::Int => "Int",
      GraphQLScalar::Float => "Float",
      GraphQLScalar::Boolean => "Boolean",
      GraphQLScalar::ID => "ID",
    }
  }
}

#[derive(Debug, Clone)]
pub enum GraphQLFieldType {
  Scalar(GraphQLScalar),
  Object(String),
  List(Box<GraphQLFieldType>),
  NonNull(Box<GraphQLFieldType>),
  Nullable(Box<GraphQLFieldType>),
}

impl GraphQLFieldType {
  pub fn to_type_string(&self) -> String {
    match self {
      GraphQLFieldType::Scalar(s) => s.as_str().to_string(),
      GraphQLFieldType::Object(name) => name.clone(),
      GraphQLFieldType::List(inner) => format!("[{}]", inner.to_type_string()),
      GraphQLFieldType::NonNull(inner) => format!("{}!", inner.to_type_string()),
      GraphQLFieldType::Nullable(inner) => inner.to_type_string(),
    }
  }
}

#[derive(Debug, Clone)]
pub struct GraphQLType {
  pub name: String,
  pub field_type: GraphQLFieldType,
  pub description: Option<String>,
  pub deprecated: Option<String>,
}
