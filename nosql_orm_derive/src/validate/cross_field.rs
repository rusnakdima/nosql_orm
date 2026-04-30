use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{DeriveInput, Meta};

pub fn generate_cross_field_validation(input: &DeriveInput) -> TokenStream2 {
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
