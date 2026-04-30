use proc_macro::TokenStream;
use syn::parse_macro_input;

mod entity;
mod validate;

use entity::{generate_entity, generate_model, generate_orm_entity_macro};
use validate::generate_validate;

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
    relations
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
    frontend_exclude
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
    relations
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
