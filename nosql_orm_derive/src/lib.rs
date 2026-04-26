use proc_macro::TokenStream;
use syn::parse_macro_input;

mod entity;
mod validate;

use entity::generate_model;
use validate::generate_validate;

#[proc_macro_derive(Model, attributes(table_name, id_field, soft_delete, timestamp, one_to_many, many_to_one, one_to_one, many_to_many))]
pub fn derive_model(input: TokenStream) -> TokenStream {
  let input = parse_macro_input!(input as syn::DeriveInput);
  generate_model(&input)
}

#[proc_macro_derive(Validate, attributes(validate))]
pub fn derive_validate(input: TokenStream) -> TokenStream {
  let input = parse_macro_input!(input as syn::DeriveInput);
  generate_validate(&input)
}
