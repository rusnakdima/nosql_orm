use proc_macro::TokenStream;
use quote::quote;
use quote::ToTokens;
use syn::{parse_macro_input, spanned::Spanned, DeriveInput};

#[proc_macro_derive(Entity, attributes(relation, table_name, id_field))]
pub fn derive_entity(input: TokenStream) -> TokenStream {
  let input = parse_macro_input!(input as DeriveInput);
  let name = &input.ident;
  let (impl_generics, ty_generics, where_clause) = &input.generics.split_for_impl();

  let mut table_name = to_snake_case(&name.to_string());
  let mut id_field = "id".to_string();
  let mut relations: Vec<(String, String, Option<String>)> = Vec::new();

  for attr in &input.attrs {
    let ident = attr
      .path()
      .get_ident()
      .map(|i| i.to_string())
      .unwrap_or_default();
    match ident.as_str() {
      "table_name" => {
        let tokens = attr.meta.to_token_stream().to_string();
        if let Ok(s) = syn::parse_str::<syn::LitStr>(&tokens) {
          table_name = s.value();
        }
      }
      "id_field" => {
        let tokens = attr.meta.to_token_stream().to_string();
        if let Ok(s) = syn::parse_str::<syn::LitStr>(&tokens) {
          id_field = s.value();
        }
      }
      "relation" => {
        if let Ok(rel) = parse_relation_from_meta(&attr.meta) {
          relations.push(rel);
        }
      }
      _ => {}
    }
  }

  let relation_defs = relations.iter().map(|r| {
    let rel_type = match r.0.as_str() {
      "one_to_one" => quote!(RelationType::OneToOne),
      "one_to_many" => quote!(RelationType::OneToMany),
      "many_to_one" => quote!(RelationType::ManyToOne),
      "many_to_many" => quote!(RelationType::ManyToMany),
      _ => quote!(RelationType::ManyToOne),
    };
    let target = &r.1;
    let fk = r.2.as_deref().unwrap_or("id");

    quote! {
        RelationDef {
            name: #target.to_string(),
            relation_type: #rel_type,
            target_collection: #target.to_string(),
            local_key: "id".to_string(),
            foreign_key: #fk.to_string(),
            join_field: None,
            local_key_in_array: None,
            transform_map_via: None,
            on_delete: None,
            cascade_soft_delete: false,
            cascade_hard_delete: false,
        }
    }
  });

  let expanded = quote! {
      impl #impl_generics ::nosql_orm::Entity for #name #ty_generics #where_clause {
          fn meta() -> ::nosql_orm::EntityMeta {
              ::nosql_orm::EntityMeta::new(#table_name).with_id_field(#id_field)
          }
      }

      impl #impl_generics ::nosql_orm::relations::WithRelations for #name #ty_generics #where_clause {
          fn relations() -> Vec<::nosql_orm::relations::RelationDef> {
              vec![#(#relation_defs),*]
          }
      }
  };

  expanded.into()
}

fn to_snake_case(s: &str) -> String {
  let mut result = String::new();
  for (i, c) in s.chars().enumerate() {
    if c.is_uppercase() && i > 0 {
      result.push('_');
    }
    result.push(c.to_lowercase().next().unwrap_or(c));
  }
  result
}

fn parse_relation_from_meta(meta: &syn::Meta) -> syn::Result<(String, String, Option<String>)> {
  let mut rel_type = None;
  let mut target = None;
  let mut foreign_key = None;

  let tokens = meta.to_token_stream().to_string();
  let content = tokens.trim().trim_matches('(').trim_matches(')');
  let parts: Vec<&str> = content.split(',').map(|s| s.trim()).collect();

  for part in parts {
    let kv: Vec<&str> = part
      .split('=')
      .map(|s| s.trim().trim_matches('"'))
      .collect();
    if kv.len() == 2 {
      let key = kv[0].trim();
      let value = kv[1].trim();
      match key {
        "one_to_one" | "one_to_many" | "many_to_one" | "many_to_many" => {
          rel_type = Some(key.to_string());
          target = Some(value.to_string());
        }
        "foreign_key" => {
          foreign_key = Some(value.to_string());
        }
        _ => {}
      }
    }
  }

  let rel_type = rel_type.ok_or_else(|| syn::Error::new(meta.span(), "Missing relation type"))?;
  let target = target.ok_or_else(|| syn::Error::new(meta.span(), "Missing target entity"))?;

  Ok((rel_type, target, foreign_key))
}

#[proc_macro_derive(Validate, attributes(validate))]
pub fn derive_validate(input: TokenStream) -> TokenStream {
  let input = parse_macro_input!(input as DeriveInput);
  let name = &input.ident;
  let (impl_generics, ty_generics, where_clause) = &input.generics.split_for_impl();

  let fields = match &input.data {
    syn::Data::Struct(s) => match &s.fields {
      syn::Fields::Named(named) => named.named.clone(),
      _ => panic!("Validate derive only supports named fields"),
    },
    _ => panic!("Validate derive only supports structs"),
  };

  let validation_blocks: Vec<_> = fields
    .iter()
    .filter_map(|field| {
      let field_name = field.ident.as_ref()?;
      let validate_attrs: Vec<_> = field
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("validate"))
        .collect();

      if validate_attrs.is_empty() {
        return None;
      }

      let checks: Vec<_> = validate_attrs
        .iter()
        .flat_map(|attr| {
          let meta = &attr.meta;
          parse_validate_meta(meta)
        })
        .collect();

      if checks.is_empty() {
        return None;
      }

      Some(generate_validation_block(field_name, &checks))
    })
    .collect();

  let expanded = quote! {
      impl #impl_generics nosql_orm::validators::Validate for #name #ty_generics #where_clause {
          fn validate(&self) -> ::nosql_orm::error::OrmResult<()> {
              #(#validation_blocks)*
              Ok(())
          }
      }
  };

  expanded.into()
}

#[derive(Clone)]
enum ValidationType {
  Email,
  Length {
    min: Option<usize>,
    max: Option<usize>,
  },
  Pattern(String),
  Range {
    min: Option<f64>,
    max: Option<f64>,
  },
  NotEmpty,
  NonNull,
}

fn parse_validate_meta(meta: &syn::Meta) -> Vec<ValidationType> {
  let mut validations = Vec::new();

  match meta {
    syn::Meta::List(list) => {
      let tokens: Vec<_> = list.tokens.clone().into_iter().collect();
      let mut i = 0;
      while i < tokens.len() {
        let token_str = tokens[i].to_string();
        match token_str.as_str() {
          "email" => {
            validations.push(ValidationType::Email);
          }
          "not_empty" | "not_empty()" => {
            validations.push(ValidationType::NotEmpty);
          }
          "non_null" | "non_null()" => {
            validations.push(ValidationType::NonNull);
          }
          "length" => {
            if i + 1 < tokens.len() {
              let args_token = tokens[i + 1].clone();
              let args_str = args_token.to_string();
              let (min, max) = parse_length_args(&args_str);
              validations.push(ValidationType::Length { min, max });
              i += 1;
            }
          }
          "pattern" => {
            if i + 1 < tokens.len() {
              let args_token = tokens[i + 1].clone();
              let args_str = args_token.to_string();
              if let Some(p) = parse_pattern_args(&args_str) {
                validations.push(ValidationType::Pattern(p));
              }
              i += 1;
            }
          }
          "range" => {
            if i + 1 < tokens.len() {
              let args_token = tokens[i + 1].clone();
              let args_str = args_token.to_string();
              let (min, max) = parse_range_args(&args_str);
              validations.push(ValidationType::Range { min, max });
              i += 1;
            }
          }
          _ => {}
        }
        i += 1;
      }
    }
    syn::Meta::NameValue(nv) => {
      let ident_str = nv
        .path
        .get_ident()
        .map(|i| i.to_string())
        .unwrap_or_default();
      match ident_str.as_str() {
        "email" => validations.push(ValidationType::Email),
        "length" => {
          if let Ok(lit) = syn::parse2::<syn::LitStr>(nv.value.to_token_stream()) {
            let (min, max) = parse_length_args(&lit.value());
            validations.push(ValidationType::Length { min, max });
          }
        }
        "pattern" => {
          if let Ok(lit) = syn::parse2::<syn::LitStr>(nv.value.to_token_stream()) {
            validations.push(ValidationType::Pattern(lit.value()));
          }
        }
        "range" => {
          if let Ok(lit) = syn::parse2::<syn::LitStr>(nv.value.to_token_stream()) {
            let (min, max) = parse_range_args(&lit.value());
            validations.push(ValidationType::Range { min, max });
          }
        }
        _ => {}
      }
    }
    _ => {}
  }

  validations
}

fn parse_length_args(args: &str) -> (Option<usize>, Option<usize>) {
  let args = args.trim().trim_start_matches('(').trim_end_matches(')');
  let mut min = None;
  let mut max = None;

  for part in args.split(',') {
    let part = part.trim();
    if part.starts_with("min = ") {
      if let Ok(v) = part[6..].parse::<usize>() {
        min = Some(v);
      }
    } else if part.starts_with("max = ") {
      if let Ok(v) = part[6..].parse::<usize>() {
        max = Some(v);
      }
    }
  }

  (min, max)
}

fn parse_range_args(args: &str) -> (Option<f64>, Option<f64>) {
  let args = args.trim().trim_start_matches('(').trim_end_matches(')');
  let mut min = None;
  let mut max = None;

  for part in args.split(',') {
    let part = part.trim();
    if part.starts_with("min = ") {
      if let Ok(v) = part[6..].parse::<f64>() {
        min = Some(v);
      }
    } else if part.starts_with("max = ") {
      if let Ok(v) = part[6..].parse::<f64>() {
        max = Some(v);
      }
    }
  }

  (min, max)
}

fn parse_pattern_args(args: &str) -> Option<String> {
  let args = args.trim().trim_start_matches('(').trim_end_matches(')');
  Some(args.to_string())
}

fn generate_validation_block(
  field_name: &syn::Ident,
  validations: &[ValidationType],
) -> proc_macro2::TokenStream {
  let checks: Vec<_> = validations
    .iter()
    .map(|v| generate_check(field_name, v))
    .collect();

  quote! {
      #(#checks)*
  }
}

fn generate_check(
  field_name: &syn::Ident,
  validation: &ValidationType,
) -> proc_macro2::TokenStream {
  match validation {
    ValidationType::Email => quote! {
        {
            let validator = ::nosql_orm::validators::EmailValidator;
            let value = ::serde_json::json!(&self.#field_name);
            ::nosql_orm::validators::FieldValidator::validate(&validator, stringify!(#field_name), &value)
                .map_err(|e| ::nosql_orm::error::OrmError::Validation(e.to_string()))?;
        }
    },
    ValidationType::NotEmpty => quote! {
        {
            let value = ::serde_json::json!(&self.#field_name);
            if value.is_null() {
                return Err(::nosql_orm::error::OrmError::Validation(
                    format!("Field '{}' cannot be null", stringify!(#field_name))
                ));
            }
            if let Some(s) = value.as_str() {
                if s.trim().is_empty() {
                    return Err(::nosql_orm::error::OrmError::Validation(
                        format!("Field '{}' cannot be empty or whitespace", stringify!(#field_name))
                    ));
                }
            }
        }
    },
    ValidationType::NonNull => quote! {
        {
            let value = ::serde_json::json!(&self.#field_name);
            if value.is_null() {
                return Err(::nosql_orm::error::OrmError::Validation(
                    format!("Field '{}' cannot be null", stringify!(#field_name))
                ));
            }
        }
    },
    ValidationType::Length { min, max } => {
      let min_check = min.map(|m| {
                quote! {
                    if s.len() < #m {
                        return Err(::nosql_orm::error::OrmError::Validation(
                            format!("Field '{}' must be at least {} characters", stringify!(#field_name), #m)
                        ));
                    }
                }
            });
      let max_check = max.map(|m| {
        quote! {
            if s.len() > #m {
                return Err(::nosql_orm::error::OrmError::Validation(
                    format!("Field '{}' must be at most {} characters", stringify!(#field_name), #m)
                ));
            }
        }
      });
      quote! {
          {
              let value = ::serde_json::json!(&self.#field_name);
              if let Some(s) = value.as_str() {
                  #min_check
                  #max_check
              }
          }
      }
    }
    ValidationType::Pattern(pattern) => quote! {
        {
            let validator = ::nosql_orm::validators::PatternValidator::new(#pattern)
                .map_err(|e| ::nosql_orm::error::OrmError::Validation(format!("Invalid pattern for '{}': {}", stringify!(#field_name), e)))?;
            let value = ::serde_json::json!(&self.#field_name);
            ::nosql_orm::validators::FieldValidator::validate(&validator, stringify!(#field_name), &value)
                .map_err(|e| ::nosql_orm::error::OrmError::Validation(e.to_string()))?;
        }
    },
    ValidationType::Range { min, max } => {
      let min_check = min.map(|m| {
        quote! {
            if n < #m as f64 {
                return Err(::nosql_orm::error::OrmError::Validation(
                    format!("Field '{}' must be at least {}", stringify!(#field_name), #m)
                ));
            }
        }
      });
      let max_check = max.map(|m| {
        quote! {
            if n > #m as f64 {
                return Err(::nosql_orm::error::OrmError::Validation(
                    format!("Field '{}' must be at most {}", stringify!(#field_name), #m)
                ));
            }
        }
      });
      quote! {
          {
              let value = ::serde_json::json!(&self.#field_name);
              if let Some(n) = value.as_f64() {
                  #min_check
                  #max_check
              }
          }
      }
    }
  }
}
