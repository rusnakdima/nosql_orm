use proc_macro::TokenStream;
use quote::quote;
use quote::ToTokens;
use syn::{parse_macro_input, DeriveInput, spanned::Spanned};

#[proc_macro_derive(Entity, attributes(relation, table_name, id_field))]
pub fn derive_entity(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = &input.generics.split_for_impl();

    let mut table_name = to_snake_case(&name.to_string());
    let mut id_field = "id".to_string();
    let mut relations: Vec<(String, String, Option<String>)> = Vec::new();

    for attr in &input.attrs {
        let ident = attr.path().get_ident().map(|i| i.to_string()).unwrap_or_default();
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
        let kv: Vec<&str> = part.split('=').map(|s| s.trim().trim_matches('"')).collect();
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
