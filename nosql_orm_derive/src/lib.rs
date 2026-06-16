use proc_macro::TokenStream;
use syn::parse_macro_input;

mod entity;
mod validate;

use entity::{generate_entity, generate_model, generate_orm_entity_macro};
use validate::generate_validate;

fn derive_model_helper(item: &syn::DeriveInput) -> Result<proc_macro::TokenStream, Vec<syn::Error>> {
    let name = &item.ident;
    let (impl_generics, ty_generics, where_clause) = &item.generics.split_for_impl();
    
    Ok(quote::quote! {
        impl #impl_generics model_helper::ModelHelper for #name #ty_generics #where_clause {
            fn validate(&self) -> Vec<model_helper::ValidationError> {
                Vec::new()
            }
            
            fn before_insert(&mut self) {}
            fn before_update(&mut self) {}
            fn after_load(&mut self) {}
            fn transform(&mut self) {}
        }
    }.into())
}

#[proc_macro_derive(ModelHelper)]
pub fn derive_model_helper_macro(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    match derive_model_helper(&input) {
        Ok(tokens) => tokens,
        Err(errors) => {
            let compile_errors = errors.iter().map(|e| e.to_compile_error());
            proc_macro::TokenStream::from(quote::quote! { #(#compile_errors)* })
        }
    }
}

#[proc_macro_derive(
  Entity,
  attributes(
    entity,
    table_name,
    id_field,
    soft_delete,
    timestamp,
    one_to_many,
    many_to_one,
    one_to_one,
    many_to_many,
    index,
    sql_column,
    frontend_exclude,
    Relations,
    relations,
    counter
  )
)]
pub fn derive_entity(input: TokenStream) -> TokenStream {
  let input = parse_macro_input!(input as syn::DeriveInput);
  generate_entity(&input)
}

#[proc_macro_derive(
  Model,
  attributes(
    table_name,
    id_field,
    soft_delete,
    timestamp,
    one_to_many,
    many_to_one,
    one_to_one,
    many_to_many,
    index,
    sql_column,
    frontend_exclude,
    Relations,
    relations,
    counter
  )
)]
pub fn derive_model(input: TokenStream) -> TokenStream {
  let input = parse_macro_input!(input as syn::DeriveInput);
  generate_model(&input)
}

#[proc_macro_derive(
  OrmEntity,
  attributes(
    table_name,
    id_field,
    soft_delete,
    timestamp,
    one_to_many,
    many_to_one,
    one_to_one,
    many_to_many,
    index,
    sql_column,
    frontend_exclude,
    Relations,
    relations,
    counter
  )
)]
pub fn derive_orm_entity(input: TokenStream) -> TokenStream {
  let input = parse_macro_input!(input as syn::DeriveInput);
  generate_orm_entity_macro(&input)
}

#[proc_macro_derive(Validate, attributes(validate))]
pub fn derive_validate(input: TokenStream) -> TokenStream {
  let input = parse_macro_input!(input as syn::DeriveInput);
  generate_validate(&input)
}
