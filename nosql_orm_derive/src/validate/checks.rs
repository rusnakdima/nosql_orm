use super::ValidationType;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::Ident;

pub fn generate_validation_block(
  field_name: &Ident,
  validations: &[ValidationType],
) -> TokenStream2 {
  let field_str = field_name.to_string();
  let checks: Vec<TokenStream2> = validations
    .iter()
    .map(|v| generate_check(&field_str, v))
    .collect();
  let mut result = TokenStream2::new();
  for check in checks {
    result.extend(check);
  }
  result
}

fn generate_check(field_str: &str, validation: &ValidationType) -> TokenStream2 {
  let field_ident = syn::Ident::new(field_str, proc_macro2::Span::call_site());
  match validation {
    ValidationType::Email => generate_email_check(&field_ident, field_str),
    ValidationType::Uuid => generate_uuid_check(&field_ident, field_str),
    ValidationType::Url => generate_url_check(&field_ident, field_str),
    ValidationType::Length(min, max) => generate_length_check(&field_ident, field_str, *min, *max),
    ValidationType::Pattern(pattern) => generate_pattern_check(&field_ident, field_str, pattern),
    ValidationType::Range(min, max) => generate_range_check(&field_ident, field_str, *min, *max),
    ValidationType::Min(min_val) => generate_min_check(&field_ident, field_str, *min_val),
    ValidationType::Max(max_val) => generate_max_check(&field_ident, field_str, *max_val),
    ValidationType::NotEmpty => generate_not_empty_check(&field_ident, field_str),
    ValidationType::NonNull => generate_non_null_check(&field_ident, field_str),
    ValidationType::Required => generate_required_check(&field_ident, field_str),
  }
}

fn generate_email_check(field_ident: &syn::Ident, field_str: &str) -> TokenStream2 {
  quote! {
      {
          let __v = serde_json::json!(&self.#field_ident);
          if let Some(__s) = __v.as_str() {
              if __s.is_empty() || !__s.contains('@') || !__s.contains('.') {
                  return Err(::nosql_orm::error::OrmError::Validation(
                      format!("{} must be a valid email address", #field_str)
                  ));
              }
          } else if !__v.is_null() {
              return Err(::nosql_orm::error::OrmError::Validation(
                  format!("{} must be a string", #field_str)
              ));
          }
      }
  }
}

fn generate_uuid_check(field_ident: &syn::Ident, field_str: &str) -> TokenStream2 {
  quote! {
      {
          let __v = serde_json::json!(&self.#field_ident);
          if let Some(__s) = __v.as_str() {
              if __s.len() != 36 || __s.chars().filter(|&c| c == '-').count() != 4 {
                  return Err(::nosql_orm::error::OrmError::Validation(
                      format!("{} must be a valid UUID (format: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx)", #field_str)
                  ));
              }
          } else if !__v.is_null() {
              return Err(::nosql_orm::error::OrmError::Validation(
                  format!("{} must be a string", #field_str)
              ));
          }
      }
  }
}

fn generate_url_check(field_ident: &syn::Ident, field_str: &str) -> TokenStream2 {
  quote! {
      {
          let __v = serde_json::json!(&self.#field_ident);
          if let Some(__s) = __v.as_str() {
              if !__s.starts_with("http://") && !__s.starts_with("https://") && !__s.starts_with("ftp://") {
                  return Err(::nosql_orm::error::OrmError::Validation(
                      format!("{} must be a valid URL starting with http://, https://, or ftp://", #field_str)
                  ));
              }
              if __s.contains(' ') || !__s.contains("://") {
                  return Err(::nosql_orm::error::OrmError::Validation(
                      format!("{} must be a valid URL", #field_str)
                  ));
              }
          } else if !__v.is_null() {
              return Err(::nosql_orm::error::OrmError::Validation(
                  format!("{} must be a string", #field_str)
              ));
          }
      }
  }
}

fn generate_length_check(
  field_ident: &syn::Ident,
  field_str: &str,
  min: Option<usize>,
  max: Option<usize>,
) -> TokenStream2 {
  let min_check = if let Some(m) = min {
    quote! {
        if __s.len() < #m {
            return Err(::nosql_orm::error::OrmError::Validation(
                format!("{} must be at least {} characters", #field_str, #m)
            ));
        }
    }
  } else {
    TokenStream2::new()
  };
  let max_check = if let Some(m) = max {
    quote! {
        if __s.len() > #m {
            return Err(::nosql_orm::error::OrmError::Validation(
                format!("{} must be at most {} characters", #field_str, #m)
            ));
        }
    }
  } else {
    TokenStream2::new()
  };
  quote! {
      {
          let __v = serde_json::json!(&self.#field_ident);
          if let Some(__s) = __v.as_str() {
              #min_check
              #max_check
          } else if !__v.is_null() {
              return Err(::nosql_orm::error::OrmError::Validation(
                  format!("{} must be a string", #field_str)
              ));
          }
      }
  }
}

fn generate_pattern_check(
  field_ident: &syn::Ident,
  field_str: &str,
  pattern: &str,
) -> TokenStream2 {
  let pattern_lit = syn::LitStr::new(pattern, proc_macro2::Span::call_site());
  quote! {
      {
          let __v = serde_json::json!(&self.#field_ident);
          if let Some(__s) = __v.as_str() {
              let __re = ::regex::Regex::new(#pattern_lit).map_err(|_| ::nosql_orm::error::OrmError::Validation("Invalid regex pattern".to_string()))?;
              if !__re.is_match(__s) {
                  return Err(::nosql_orm::error::OrmError::Validation(
                      format!("{} must match pattern {}", #field_str, #pattern_lit)
                  ));
              }
          } else if !__v.is_null() {
              return Err(::nosql_orm::error::OrmError::Validation(
                  format!("{} must be a string", #field_str)
              ));
          }
      }
  }
}

fn generate_range_check(
  field_ident: &syn::Ident,
  field_str: &str,
  min: Option<f64>,
  max: Option<f64>,
) -> TokenStream2 {
  let min_check = if let Some(m) = min {
    quote! {
        if (__v.as_f64().unwrap_or(0.0) as f64) < #m {
            return Err(::nosql_orm::error::OrmError::Validation(
                format!("{} must be at least {}", #field_str, #m)
            ));
        }
    }
  } else {
    TokenStream2::new()
  };
  let max_check = if let Some(m) = max {
    quote! {
        if (__v.as_f64().unwrap_or(0.0) as f64) > #m {
            return Err(::nosql_orm::error::OrmError::Validation(
                format!("{} must be at most {}", #field_str, #m)
            ));
        }
    }
  } else {
    TokenStream2::new()
  };
  quote! {
      {
          let __v = serde_json::json!(&self.#field_ident);
          if __v.is_number() {
              #min_check
              #max_check
          } else if !__v.is_null() {
              return Err(::nosql_orm::error::OrmError::Validation(
                  format!("{} must be a number", #field_str)
              ));
          }
      }
  }
}

fn generate_min_check(field_ident: &syn::Ident, field_str: &str, min_val: f64) -> TokenStream2 {
  quote! {
      {
          let __v = serde_json::json!(&self.#field_ident);
          if __v.is_number() {
              if let Some(__n) = __v.as_f64() {
                  if __n < #min_val {
                      return Err(::nosql_orm::error::OrmError::Validation(
                          format!("{} must be at least {}", #field_str, #min_val)
                      ));
                  }
              }
          } else if !__v.is_null() {
              return Err(::nosql_orm::error::OrmError::Validation(
                  format!("{} must be a number", #field_str)
              ));
          }
      }
  }
}

fn generate_max_check(field_ident: &syn::Ident, field_str: &str, max_val: f64) -> TokenStream2 {
  quote! {
      {
          let __v = serde_json::json!(&self.#field_ident);
          if __v.is_number() {
              if let Some(__n) = __v.as_f64() {
                  if __n > #max_val {
                      return Err(::nosql_orm::error::OrmError::Validation(
                          format!("{} must be at most {}", #field_str, #max_val)
                      ));
                  }
              }
          } else if !__v.is_null() {
              return Err(::nosql_orm::error::OrmError::Validation(
                  format!("{} must be a number", #field_str)
              ));
          }
      }
  }
}

fn generate_not_empty_check(field_ident: &syn::Ident, field_str: &str) -> TokenStream2 {
  quote! {
      {
          let __v = serde_json::json!(&self.#field_ident);
          if let Some(__s) = __v.as_str() {
              if __s.trim().is_empty() {
                  return Err(::nosql_orm::error::OrmError::Validation(
                      format!("{} cannot be empty", #field_str)
                  ));
              }
          }
      }
  }
}

fn generate_non_null_check(field_ident: &syn::Ident, field_str: &str) -> TokenStream2 {
  quote! {
      {
          let __v = serde_json::json!(&self.#field_ident);
          if __v.is_null() {
              return Err(::nosql_orm::error::OrmError::Validation(
                  format!("{} cannot be null", #field_str)
              ));
          }
      }
  }
}

fn generate_required_check(field_ident: &syn::Ident, field_str: &str) -> TokenStream2 {
  quote! {
      {
          let __v = serde_json::json!(&self.#field_ident);
          if __v.is_null() || (__v.is_string() && __v.as_str().unwrap_or("").is_empty()) {
              return Err(::nosql_orm::error::OrmError::Validation(
                  format!("{} is required", #field_str)
              ));
          }
      }
  }
}
