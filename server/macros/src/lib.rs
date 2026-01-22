use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, parse_macro_input};

#[proc_macro_attribute]
pub fn non_nullable_options(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(item as DeriveInput);

    if let Data::Struct(data_struct) = &mut input.data
        && let Fields::Named(fields) = &mut data_struct.fields
    {
        for field in fields.named.iter_mut() {
            // Check if field already has a #[schema(nullable...)] attribute
            let has_nullable = field.attrs.iter().any(|attr| {
                attr.path().is_ident("schema")
                    && attr
                        .meta
                        .require_list()
                        .ok()
                        .map_or(false, |list| list.tokens.to_string().contains("nullable"))
            });

            // Only add nullable = false if not already specified
            if !has_nullable {
                field.attrs.push(syn::parse_quote!(#[schema(nullable = false)]));
            }
        }
    }

    TokenStream::from(quote!(#input))
}
