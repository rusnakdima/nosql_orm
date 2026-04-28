use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use quote::ToTokens;
use syn::{DeriveInput, Ident, Meta};

pub fn generate_validate(input: &DeriveInput) -> TokenStream {
  let name = &input.ident;
  let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

  let fields = match &input.data {
    syn::Data::Struct(s) => match &s.fields {
      syn::Fields::Named(n) => n.named.clone(),
      _ => panic!("Validate derive only supports named fields"),
    },
    _ => panic!("Validate derive only supports structs"),
  };

  let blocks: Vec<_> = fields
    .iter()
    .filter_map(|f| {
      let fname = f.ident.as_ref()?;
      let vas: Vec<_> = f
        .attrs
        .iter()
        .filter(|a| a.path().is_ident("validate"))
        .collect();
      if vas.is_empty() {
        return None;
      }
      let checks: Vec<_> = vas
        .iter()
        .flat_map(|a| parse_validation_meta(&a.meta))
        .collect();
      if checks.is_empty() {
        return None;
      }
      Some(generate_validation_block(fname, &checks))
    })
    .collect();

  let cross_field_blocks = generate_cross_field_validation(input);

  quote! {
      impl #impl_generics nosql_orm::validators::Validate for #name #ty_generics #where_clause {
          fn validate(&self) -> ::nosql_orm::error::OrmResult<()> {
              #(#blocks)*
              #cross_field_blocks
              Ok(())
          }
      }
  }
  .into()
}

#[derive(Clone)]
enum ValidationType {
  Email,
  Uuid,
  Url,
  Length(Option<usize>, Option<usize>),
  Pattern(String),
  Range(Option<f64>, Option<f64>),
  Min(f64),
  Max(f64),
  NotEmpty,
  NonNull,
  Required,
}

pub fn parse_validation_meta(meta: &Meta) -> Vec<ValidationType> {
  let mut validations = Vec::new();
  match meta {
    Meta::List(list) => {
      let tokens: Vec<_> = list.tokens.clone().into_iter().collect();
      let mut i = 0;
      while i < tokens.len() {
        let token_str = tokens[i].to_string();
        match token_str.as_str() {
          "email" => validations.push(ValidationType::Email),
          "uuid" => validations.push(ValidationType::Uuid),
          "url" => validations.push(ValidationType::Url),
          "not_empty" => validations.push(ValidationType::NotEmpty),
          "non_null" => validations.push(ValidationType::NonNull),
          "required" => validations.push(ValidationType::Required),
          "length" => {
            if i + 1 < tokens.len() {
              let (mn, mx) = parse_length_args(&tokens[i + 1].to_string());
              validations.push(ValidationType::Length(mn, mx));
              i += 1
            }
          }
          "min" => {
            if i + 1 < tokens.len() {
              if let Ok(x) = tokens[i + 1]
                .to_string()
                .trim()
                .trim_matches('(')
                .trim_matches(')')
                .parse::<f64>()
              {
                validations.push(ValidationType::Min(x));
              }
              i += 1
            }
          }
          "max" => {
            if i + 1 < tokens.len() {
              if let Ok(x) = tokens[i + 1]
                .to_string()
                .trim()
                .trim_matches('(')
                .trim_matches(')')
                .parse::<f64>()
              {
                validations.push(ValidationType::Max(x));
              }
              i += 1
            }
          }
          "pattern" => {
            if i + 1 < tokens.len() {
              if let Some(p) = parse_pattern_arg(&tokens[i + 1].to_string()) {
                validations.push(ValidationType::Pattern(p));
              }
              i += 1
            }
          }
          "range" => {
            if i + 1 < tokens.len() {
              let (mn, mx) = parse_range_args(&tokens[i + 1].to_string());
              validations.push(ValidationType::Range(mn, mx));
              i += 1
            }
          }
          _ => {}
        }
        i += 1;
      }
    }
    Meta::NameValue(nv) => {
      let ident_str = nv
        .path
        .get_ident()
        .map(|i| i.to_string())
        .unwrap_or_default();
      match ident_str.as_str() {
        "email" => validations.push(ValidationType::Email),
        "uuid" => validations.push(ValidationType::Uuid),
        "url" => validations.push(ValidationType::Url),
        "required" => validations.push(ValidationType::Required),
        "length" => {
          if let Ok(lit) = syn::parse2::<syn::LitStr>(nv.value.to_token_stream()) {
            let (mn, mx) = parse_length_args(&lit.value());
            validations.push(ValidationType::Length(mn, mx))
          }
        }
        "min" => {
          if let Ok(lit) = syn::parse2::<syn::LitStr>(nv.value.to_token_stream()) {
            if let Ok(x) = lit.value().parse::<f64>() {
              validations.push(ValidationType::Min(x))
            }
          }
        }
        "max" => {
          if let Ok(lit) = syn::parse2::<syn::LitStr>(nv.value.to_token_stream()) {
            if let Ok(x) = lit.value().parse::<f64>() {
              validations.push(ValidationType::Max(x))
            }
          }
        }
        "pattern" => {
          if let Ok(lit) = syn::parse2::<syn::LitStr>(nv.value.to_token_stream()) {
            validations.push(ValidationType::Pattern(lit.value()))
          }
        }
        "range" => {
          if let Ok(lit) = syn::parse2::<syn::LitStr>(nv.value.to_token_stream()) {
            let (mn, mx) = parse_range_args(&lit.value());
            validations.push(ValidationType::Range(mn, mx))
          }
        }
        _ => {}
      }
    }
    _ => {}
  }
  validations
}

fn generate_cross_field_validation(input: &DeriveInput) -> TokenStream2 {
  let mut cross_field_blocks = TokenStream2::new();

  for attr in &input.attrs {
    if attr.path().is_ident("validate") {
      let meta_list = match &attr.meta {
        Meta::List(list) => list,
        _ => continue,
      };

      let tokens: Vec<_> = meta_list.tokens.clone().into_iter().collect();
      let mut i = 0;
      while i < tokens.len() {
        let token_str = tokens[i].to_string();
        match token_str.as_str() {
          "xor" => {
            if i + 2 < tokens.len() {
              let field1 = tokens[i + 1].to_string().trim().to_string();
              let field2 = tokens[i + 2].to_string().trim().to_string();
              let block = generate_xor_check(&field1, &field2);
              cross_field_blocks.extend(block);
              i += 3;
            } else {
              i += 1;
            }
          }
          "require_one_of" => {
            let mut field_names = Vec::new();
            let mut j = i + 1;
            while j < tokens.len() && tokens[j].to_string() != "," {
              let f = tokens[j].to_string().trim().to_string();
              if !f.is_empty() {
                field_names.push(f);
              }
              j += 1;
            }
            if !field_names.is_empty() {
              let block = generate_require_one_of_check(&field_names);
              cross_field_blocks.extend(block);
              i = j + 1;
            } else {
              i += 1;
            }
          }
          "if_then" => {
            if i + 3 < tokens.len() {
              let cond = tokens[i + 1].to_string().trim().to_string();
              let then_field = tokens[i + 3].to_string().trim().to_string();
              let block = generate_if_then_check(&cond, &then_field);
              cross_field_blocks.extend(block);
              i += 4;
            } else {
              i += 1;
            }
          }
          _ => i += 1,
        }
      }
    }
  }

  cross_field_blocks
}

fn generate_xor_check(field1: &str, field2: &str) -> TokenStream2 {
  let f1_ident = syn::Ident::new(field1, proc_macro2::Span::call_site());
  let f2_ident = syn::Ident::new(field2, proc_macro2::Span::call_site());
  let f1_str = field1.to_string();
  let f2_str = field2.to_string();

  quote! {
    {
      let __v1 = serde_json::json!(&self.#f1_ident);
      let __v2 = serde_json::json!(&self.#f2_ident);
      let __has_1 = !__v1.is_null() && !(__v1.is_string() && __v1.as_str().unwrap_or("").is_empty());
      let __has_2 = !__v2.is_null() && !(__v2.is_string() && __v2.as_str().unwrap_or("").is_empty());
      if __has_1 == __has_2 {
        return Err(::nosql_orm::error::OrmError::Validation(
          format!("{} and {} are mutually exclusive - exactly one must be provided", #f1_str, #f2_str)
        ));
      }
    }
  }
}

fn generate_require_one_of_check(field_names: &[String]) -> TokenStream2 {
  let field_names_str = field_names.join(", ");

  if field_names.len() == 1 {
    let ident = syn::Ident::new(&field_names[0], proc_macro2::Span::call_site());
    return quote! {
      {
        let __v = serde_json::json!(&self.#ident);
        if __v.is_null() || (__v.is_string() && __v.as_str().unwrap_or("").is_empty()) {
          return Err(::nosql_orm::error::OrmError::Validation(
            format!("{} is required", #field_names_str)
          ));
        }
      }
    };
  }

  let mut check_blocks = Vec::new();
  for fname in field_names {
    let ident = syn::Ident::new(fname, proc_macro2::Span::call_site());
    check_blocks.push(quote! {
      let __v = serde_json::json!(&self.#ident);
      !__v.is_null() && !(__v.is_string() && __v.as_str().unwrap_or("").is_empty())
    });
  }

  let first_check = check_blocks[0].clone();
  let rest_checks: Vec<_> = check_blocks.iter().skip(1).collect();

  let mut combined = first_check;
  for check in rest_checks {
    combined = quote! { #combined || #check };
  }

  quote! {
    {
      if !(#combined) {
        return Err(::nosql_orm::error::OrmError::Validation(
          format!("at least one of [{}] must be provided", #field_names_str)
        ));
      }
    }
  }
}

fn generate_if_then_check(cond_field: &str, then_field: &str) -> TokenStream2 {
  let cond_ident = syn::Ident::new(cond_field, proc_macro2::Span::call_site());
  let then_ident = syn::Ident::new(then_field, proc_macro2::Span::call_site());
  let cond_str = cond_field.to_string();
  let then_str = then_field.to_string();

  quote! {
    {
      let __cond_v = serde_json::json!(&self.#cond_ident);
      let __then_v = serde_json::json!(&self.#then_ident);
      if !__cond_v.is_null() && (__then_v.is_null() || (__then_v.is_string() && __then_v.as_str().unwrap_or("").is_empty())) {
        return Err(::nosql_orm::error::OrmError::Validation(
          format!("{} is required when {} is set", #then_str, #cond_str)
        ));
      }
    }
  }
}

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
    ValidationType::Email => {
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
    ValidationType::Uuid => {
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
    ValidationType::Url => {
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
    ValidationType::Length(min, max) => {
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
    ValidationType::Pattern(pattern) => {
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
    ValidationType::Range(min, max) => {
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
    ValidationType::Min(min_val) => {
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
    ValidationType::Max(max_val) => {
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
    ValidationType::NotEmpty => {
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
    ValidationType::NonNull => {
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
    ValidationType::Required => {
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
  }
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

fn parse_pattern_arg(args: &str) -> Option<String> {
  Some(
    args
      .trim()
      .trim_start_matches('(')
      .trim_end_matches(')')
      .to_string(),
  )
}