use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, Meta};

pub fn generate_model(input: &DeriveInput) -> TokenStream {
  let name = &input.ident;
  let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

  let mut table_name = to_snake_case(&name.to_string());
  let mut id_field = "id".to_string();
  let mut has_soft_delete = false;
  let mut relations: Vec<proc_macro2::TokenStream> = Vec::new();

  for attr in &input.attrs {
    let meta = &attr.meta;
    match meta {
      Meta::NameValue(nv) => {
        let ident = nv.path.get_ident().map(|i| i.to_string()).unwrap_or_default();
        if ident == "table_name" {
          if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) = &nv.value {
            table_name = s.value();
          }
        } else if ident == "id_field" {
          if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) = &nv.value {
            id_field = s.value();
          }
        }
      }
      Meta::Path(p) => {
        if p.is_ident("soft_delete") {
          has_soft_delete = true;
        }
      }
      Meta::List(list) => {
        let ident = list.path.get_ident().map(|i| i.to_string()).unwrap_or_default();
        if ident == "one_to_many" || ident == "many_to_one" || ident == "one_to_one" || ident == "many_to_many" {
          let rel = parse_relation_attr(&ident, &list.tokens);
          if let Some(r) = rel {
            relations.push(r);
          }
        }
      }
      _ => {}
    }
  }

  let entity_impl = quote! {
      impl #impl_generics ::nosql_orm::Entity for #name #ty_generics #where_clause {
          fn meta() -> ::nosql_orm::EntityMeta {
              ::nosql_orm::EntityMeta::new(#table_name).with_id_field(#id_field)
          }
          fn get_id(&self) -> Option<String> { self.id.clone() }
          fn set_id(&mut self, id: String) { self.id = Some(id); }
          fn is_soft_deletable() -> bool { #has_soft_delete }
      }
  };

  if relations.is_empty() {
    entity_impl.into()
  } else {
    let with_relations_impl = quote! {
        impl #impl_generics ::nosql_orm::relations::WithRelations for #name #ty_generics #where_clause {
            fn relations() -> Vec<::nosql_orm::relations::RelationDef> {
                vec![
                    #(#relations),*
                ]
            }
        }
    };
    quote! {
        #entity_impl
        #with_relations_impl
    }
    .into()
  }
}

fn parse_relation_attr(rel_type: &str, tokens: &proc_macro2::TokenStream) -> Option<proc_macro2::TokenStream> {
  let ts_str = tokens.to_string();
  let parts: Vec<&str> = ts_str.split(',').map(|s| s.trim().trim_matches('"')).collect();
  
  let name = parts.get(0)?;
  let target = parts.get(1).unwrap_or(&"");
  let key = parts.get(2).unwrap_or(&"id");
  
  let on_delete_str = parts.get(3).map(|s| *s).unwrap_or("");
  let on_delete = if on_delete_str.contains("Cascade") {
    quote! { ::nosql_orm::sql::types::SqlOnDelete::Cascade }
  } else if on_delete_str.contains("Null") {
    quote! { ::nosql_orm::sql::types::SqlOnDelete::SetNull }
  } else if on_delete_str.contains("Restrict") {
    quote! { ::nosql_orm::sql::types::SqlOnDelete::Restrict }
  } else {
    quote! { ::nosql_orm::sql::types::SqlOnDelete::NoAction }
  };

  match rel_type {
    "one_to_many" => Some(quote! {
        ::nosql_orm::relations::RelationDef::one_to_many(#name, #target, #key).on_delete(#on_delete)
    }),
    "many_to_one" => Some(quote! {
        ::nosql_orm::relations::RelationDef::many_to_one(#name, #target, #key)
    }),
    "one_to_one" => Some(quote! {
        ::nosql_orm::relations::RelationDef::one_to_one(#name, #target, #key)
    }),
    "many_to_many" => Some(quote! {
        ::nosql_orm::relations::RelationDef::many_to_many(#name, #target, #key)
    }),
    _ => None
  }
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