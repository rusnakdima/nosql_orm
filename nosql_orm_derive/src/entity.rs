use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, Meta};

pub fn generate_entity(input: &DeriveInput) -> TokenStream {
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let mut table_name = to_snake_case(&name.to_string());
    let mut id_field = "id".to_string();
    let mut has_soft_delete = false;
    let mut relations: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut indexes: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut sql_columns: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut frontend_excluded_fields: Vec<String> = Vec::new();
    let mut timestamps = false;

    let mut has_deleted_at_field = false;
    if let syn::Data::Struct(struct_data) = &input.data {
        for field in &struct_data.fields {
            if let Some(ident) = &field.ident {
                match ident.to_string().as_str() {
                    "deleted_at" => has_deleted_at_field = true,
                    "created_at" => timestamps = true,
                    "updated_at" => timestamps = true,
                    _ => {}
                }
            }
        }
    }

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
                } else if ident == "index" {
                    if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) = &nv.value {
                        let idx = parse_index_attr(&s.value());
                        if let Some(i) = idx {
                            indexes.push(i);
                        }
                    }
                } else if ident == "sql_column" {
                    if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) = &nv.value {
                        let col = parse_sql_column_attr(&s.value());
                        if let Some(c) = col {
                            sql_columns.push(c);
                        }
                    }
                } else if ident == "frontend_exclude" {
                    if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) = &nv.value {
                        frontend_excluded_fields.push(s.value());
                    }
                } else if ident == "entity" {
                    if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) = &nv.value {
                        table_name = s.value();
                    }
                }
            }
            Meta::Path(p) => {
                let ident = p.get_ident().map(|i| i.to_string()).unwrap_or_default();
                if ident == "soft_delete" {
                    has_soft_delete = true;
                } else if ident == "timestamp" {
                    timestamps = true;
                }
            }
            Meta::List(list) => {
                let ident = list.path.get_ident().map(|i| i.to_string()).unwrap_or_default();
                if ident == "entity" {
                    if let Ok(s) = list.parse_args::<syn::LitStr>() {
                        table_name = s.value();
                    }
                } else if ident == "table_name" {
                    if let Ok(s) = list.parse_args::<syn::LitStr>() {
                        table_name = s.value();
                    }
                } else if ident == "id_field" {
                    let ts_str = list.tokens.to_string();
                    let field = ts_str.trim().trim_matches('"').trim().to_string();
                    if !field.is_empty() {
                        id_field = field;
                    }
                } else if ident == "one_to_many" || ident == "many_to_one" || ident == "one_to_one" || ident == "many_to_many" || ident == "many_to_one_array" {
                    let rel = parse_relation_attr(&ident, &list.tokens);
                    if let Some(r) = rel {
                        relations.push(r);
                    }
                } else if ident == "index" {
                    let idx = parse_index_list(&list.tokens);
                    if let Some(i) = idx {
                        indexes.push(i);
                    }
                } else if ident == "sql_column" {
                    let col = parse_sql_column_list(&list.tokens);
                    if let Some(c) = col {
                        sql_columns.push(c);
                    }
                } else if ident == "frontend_exclude" {
                    let ts_str = list.tokens.to_string();
                    for field in ts_str.split(',') {
                        let field = field.trim().trim_matches('"').trim().to_string();
                        if !field.is_empty() {
                            frontend_excluded_fields.push(field);
                        }
                    }
                } else if ident == "Relations" || ident == "relations" {
                    let rels = parse_relations_attr(&list.tokens);
                    relations.extend(rels);
                }
            }
        }
    }

    let has_soft_delete_attr = has_soft_delete || has_deleted_at_field;
    let timestamps_attr = timestamps;

    let indexes_impl = if indexes.is_empty() {
        quote! { Vec::new() }
    } else {
        quote! { vec![#(#indexes),*] }
    };

    let sql_columns_impl = if sql_columns.is_empty() {
        quote! { Vec::new() }
    } else {
        quote! { vec![#(#sql_columns),*] }
    };

    let relations_impl = if relations.is_empty() {
        quote! { vec![] }
    } else {
        quote! { vec![#(#relations),*] }
    };

    let mut all_outputs: Vec<proc_macro2::TokenStream> = Vec::new();

    let id_field_ident = syn::Ident::new(&id_field, proc_macro2::Span::call_site());
    let entity_impl = quote! {
        impl #impl_generics ::nosql_orm::Entity for #name #ty_generics #where_clause {
            fn meta() -> ::nosql_orm::EntityMeta {
                ::nosql_orm::EntityMeta::new(#table_name).with_id_field(#id_field)
            }
            fn get_id(&self) -> Option<String> { self.#id_field_ident.clone() }
            fn set_id(&mut self, id: String) { self.#id_field_ident = Some(id); }
            fn is_soft_deletable() -> bool { #has_soft_delete_attr }
            fn indexes() -> Vec<::nosql_orm::nosql_index::NosqlIndex> { #indexes_impl }
            fn sql_columns() -> Vec<::nosql_orm::sql::SqlColumnDef> { #sql_columns_impl }
        }
    };
    all_outputs.push(entity_impl);

    let relations_impl_block = quote! {
        impl #impl_generics ::nosql_orm::relations::WithRelations for #name #ty_generics #where_clause {
            fn relations() -> Vec<::nosql_orm::relations::RelationDef> {
                #relations_impl
            }
        }
    };
    all_outputs.push(relations_impl_block);

    if has_soft_delete_attr {
        let soft_delete_impl = quote! {
            impl #impl_generics ::nosql_orm::SoftDeletable for #name #ty_generics #where_clause {
                fn deleted_at(&self) -> Option<::chrono::DateTime<::chrono::Utc>> {
                    self.deleted_at
                }
                fn set_deleted_at(&mut self, d: Option<::chrono::DateTime<::chrono::Utc>>) {
                    self.deleted_at = d;
                }
            }
        };
        all_outputs.push(soft_delete_impl);
    }

    if timestamps_attr {
        let timestamps_impl = quote! {
            impl #impl_generics ::nosql_orm::Timestamps for #name #ty_generics #where_clause {
                fn created_at(&self) -> Option<::chrono::DateTime<::chrono::Utc>> {
                    self.created_at
                }
                fn updated_at(&self) -> Option<::chrono::DateTime<::chrono::Utc>> {
                    self.updated_at
                }
                fn set_created_at(&mut self, d: ::chrono::DateTime<::chrono::Utc>) {
                    self.created_at = Some(d);
                }
                fn set_updated_at(&mut self, d: ::chrono::DateTime<::chrono::Utc>) {
                    self.updated_at = Some(d);
                }
                fn apply_timestamps_for_insert(&mut self) {
                    let now = ::chrono::Utc::now();
                    if self.created_at.is_none() {
                        self.created_at = Some(now);
                    }
                    if self.updated_at.is_none() {
                        self.updated_at = Some(now);
                    }
                }
                fn apply_timestamps_for_update(&mut self) {
                    self.updated_at = Some(::chrono::Utc::now());
                }
            }
        };
        all_outputs.push(timestamps_impl);
    }

    let _ = table_name;
    if !relations.is_empty() {
        // Auto-registration can be done manually via:
        // register_collection_relations("table_name", EntityName::relations());
        // For now, relations will be registered when entity is first used via WithRelations trait
    }

    let frontend_impl = if !frontend_excluded_fields.is_empty() {
        let fields: Vec<&str> = frontend_excluded_fields.iter().map(|s| s.as_str()).collect();
        Some(quote! {
            impl #impl_generics ::nosql_orm::FrontendProjection for #name #ty_generics #where_clause {
                fn frontend_excluded_fields() -> Vec<&'static str> {
                    vec![#(#fields),*]
                }
            }
        })
    } else {
        None
    };

    if let Some(front_impl) = frontend_impl {
        all_outputs.push(front_impl);
    }

    quote! { #(#all_outputs)* }.into()
}

fn parse_relations_attr(tokens: &proc_macro2::TokenStream) -> Vec<proc_macro2::TokenStream> {
    let ts_str = tokens.to_string();
    let rels: Vec<&str> = ts_str.split(',')
        .map(|s| s.trim().trim_matches('"').trim())
        .filter(|s| !s.is_empty())
        .collect();

    rels.iter().map(|rel_name| {
        let collection = to_snake_case(rel_name);
        let key = format!("{}_id", rel_name.to_lowercase());
        quote! {
            ::nosql_orm::relations::RelationDef::one_to_many(#rel_name, #collection, #key)
        }
    }).collect()
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
        "one_to_many" => {
          let mut rel = quote! {
              ::nosql_orm::relations::RelationDef::one_to_many(#name, #target, #key).on_delete(#on_delete)
          };
          if let Some(cascade_soft) = parse_cascade_attr(&ts_str, "cascade_soft_delete") {
            rel = quote! { #rel.cascade_soft_delete(#cascade_soft) };
          }
          if let Some(cascade_hard) = parse_cascade_attr(&ts_str, "cascade_hard_delete") {
            rel = quote! { #rel.cascade_hard_delete(#cascade_hard) };
          }
          Some(rel)
        }
        "many_to_one" => Some(quote! {
            ::nosql_orm::relations::RelationDef::many_to_one(#name, #target, #key)
        }),
        "one_to_one" => Some(quote! {
            ::nosql_orm::relations::RelationDef::one_to_one(#name, #target, #key)
        }),
        "many_to_many" => Some(quote! {
            ::nosql_orm::relations::RelationDef::many_to_many(#name, #target, #key)
        }),
        "many_to_one_array" => Some(quote! {
            ::nosql_orm::relations::RelationDef::many_to_one_array(#name, #target, #key)
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

fn parse_index_attr(s: &str) -> Option<proc_macro2::TokenStream> {
    let parts: Vec<&str> = s.split(',').map(|p| p.trim().trim_matches('"')).collect();
    let field = parts.get(0)?.trim();
    let order: i32 = parts.get(1).and_then(|o| o.parse().ok()).unwrap_or(1);
    let unique = parts.get(2).map(|u| *u == "unique").unwrap_or(false);

    if unique {
        Some(quote! { ::nosql_orm::nosql_index::NosqlIndex::single(#field, #order).unique(true) })
    } else {
        Some(quote! { ::nosql_orm::nosql_index::NosqlIndex::single(#field, #order) })
    }
}

fn parse_index_list(tokens: &proc_macro2::TokenStream) -> Option<proc_macro2::TokenStream> {
    let ts_str = tokens.to_string();
    parse_index_attr(&ts_str)
}

fn parse_sql_column_attr(s: &str) -> Option<proc_macro2::TokenStream> {
    let parts: Vec<&str> = s.split(',').map(|p| p.trim().trim_matches('"')).collect();
    let name = parts.get(0)?.trim();
    let col_type = parts.get(1).map(|t| t.trim()).unwrap_or("text");
    let is_unique = parts.get(2).map(|u| *u == "unique").unwrap_or(false);
    let is_primary = parts.get(2).map(|p| *p == "primary").unwrap_or(false);

    let sql_col_type = match col_type {
        "serial" => quote! { ::nosql_orm::sql::SqlColumnType::Serial },
        "bigserial" => quote! { ::nosql_orm::sql::SqlColumnType::BigSerial },
        "boolean" => quote! { ::nosql_orm::sql::SqlColumnType::Boolean },
        "integer" => quote! { ::nosql_orm::sql::SqlColumnType::Integer },
        "bigint" => quote! { ::nosql_orm::sql::SqlColumnType::BigInteger },
        "smallint" => quote! { ::nosql_orm::sql::SqlColumnType::SmallInteger },
        "float" => quote! { ::nosql_orm::sql::SqlColumnType::Float },
        "double" => quote! { ::nosql_orm::sql::SqlColumnType::Double },
        "varchar" => {
            let size: usize = parts.get(3).and_then(|s| s.parse().ok()).unwrap_or(255);
            quote! { ::nosql_orm::sql::SqlColumnType::VarChar(#size) }
        }
        "char" => {
            let size: usize = parts.get(3).and_then(|s| s.parse().ok()).unwrap_or(1);
            quote! { ::nosql_orm::sql::SqlColumnType::Char(#size) }
        }
        "text" => quote! { ::nosql_orm::sql::SqlColumnType::Text },
        "date" => quote! { ::nosql_orm::sql::SqlColumnType::Date },
        "time" => quote! { ::nosql_orm::sql::SqlColumnType::Time },
        "datetime" => quote! { ::nosql_orm::sql::SqlColumnType::DateTime },
        "timestamp" => quote! { ::nosql_orm::sql::SqlColumnType::Timestamp },
        "json" => quote! { ::nosql_orm::sql::SqlColumnType::Json },
        "jsonb" => quote! { ::nosql_orm::sql::SqlColumnType::JsonB },
        "uuid" => quote! { ::nosql_orm::sql::SqlColumnType::Uuid },
        _ => quote! { ::nosql_orm::sql::SqlColumnType::Text },
    };

    let mut col = quote! { ::nosql_orm::sql::SqlColumnDef::new(#name, #sql_col_type) };
    if is_primary {
        col = quote! { #col.primary_key() };
    } else if is_unique {
        col = quote! { #col.unique() };
    }
    Some(col)
}

fn parse_sql_column_list(tokens: &proc_macro2::TokenStream) -> Option<proc_macro2::TokenStream> {
    let ts_str = tokens.to_string();
    parse_sql_column_attr(&ts_str)
}

pub fn generate_model(input: &DeriveInput) -> TokenStream {
    generate_entity(input)
}

fn parse_cascade_attr(ts_str: &str, attr_name: &str) -> Option<bool> {
  if ts_str.contains(attr_name) {
    Some(ts_str.contains(&format!("{}(true)", attr_name)) || ts_str.contains(&format!("{} = true", attr_name)))
  } else {
    None
  }
}