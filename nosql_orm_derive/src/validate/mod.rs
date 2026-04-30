use proc_macro::TokenStream;
use quote::quote;
use syn::DeriveInput;

mod checks;
mod cross_field;
mod parse;

pub use checks::generate_validation_block;
pub use cross_field::generate_cross_field_validation;
pub use parse::parse_validation_meta;

#[derive(Clone)]
pub(crate) enum ValidationType {
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
