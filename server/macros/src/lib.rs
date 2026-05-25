use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, GenericArgument, PathArguments, Type, parse_macro_input};

/// Types whose getters return by value (must be `Copy`) instead of by reference.
const BY_VALUE_TYPES: &[&str] = &[
    "bool",
    "i8",
    "i16",
    "i32",
    "i64",
    "i128",
    "u8",
    "u16",
    "u32",
    "u64",
    "u128",
    "AvatarStyle",
    "DateTime",
    "MimeType",
    "PostFlags",
    "PostSafety",
    "PostType",
    "Rating",
    "ResourceOperation",
    "ResourceType",
    "UserRank",
    "Uuid",
];

/// Returns the inner type `T` if `ty` is an `Option<T>`, otherwise `None`.
/// Syntactic match only — recognizes `Option<T>`, `std::option::Option<T>`,
/// and `core::option::Option<T>`, but not type aliases.
fn option_inner_type(ty: &Type) -> Option<&Type> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    if type_path.qself.is_some() {
        return None;
    }
    let segment = type_path.path.segments.last()?;
    if segment.ident != "Option" {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    args.args.iter().find_map(|arg| match arg {
        GenericArgument::Type(inner) => Some(inner),
        _ => None,
    })
}

/// True if `ty`'s final path segment matches one of the given names and has no
/// generic arguments (e.g. `i64` or `PostType`, not `Foo<i64>`).
/// Syntactic match only — does not see through type aliases.
fn type_matches(ty: &Type, names: &[&str]) -> bool {
    let Type::Path(type_path) = ty else {
        return false;
    };
    let Some(segment) = type_path.path.segments.last() else {
        return false;
    };
    if !matches!(segment.arguments, PathArguments::None) {
        return false;
    }
    names.iter().any(|name| segment.ident == name)
}

#[proc_macro_attribute]
pub fn resource(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(item as DeriveInput);

    // Collected getter methods to emit in an impl block
    let mut getters = Vec::new();

    if let Data::Struct(data_struct) = &mut input.data
        && let Fields::Named(fields) = &mut data_struct.fields
    {
        for field in fields.named.iter_mut() {
            // Safe: Fields::Named always have idents
            let ident = field.ident.clone().unwrap();

            // The original (pre-wrap) type is what the getter exposes
            let original_ty = field.ty.clone();
            let was_option = option_inner_type(&original_ty).is_some();

            // Carry the field's doc comments onto the getter
            let docs: Vec<_> = field
                .attrs
                .iter()
                .filter(|attr| attr.path().is_ident("doc"))
                .cloned()
                .collect();

            // The name carried in the error: a single trailing `_` stripped so
            // `type_` reports as `type`. This is only the error message string —
            // the getter method name stays as the verbatim field ident
            let field_name = {
                let name = ident.to_string();
                name.strip_suffix('_').unwrap_or(&name).to_owned()
            };

            // Return by value for Copy types in the allow-list, by reference otherwise
            let getter = if type_matches(&original_ty, BY_VALUE_TYPES) {
                quote! {
                    #(#docs)*
                    pub fn #ident(&self) -> ::core::result::Result<#original_ty, crate::resource::NotRequested> {
                        self.#ident.ok_or(crate::resource::NotRequested(#field_name))
                    }
                }
            } else {
                quote! {
                    #(#docs)*
                    pub fn #ident(&self) -> ::core::result::Result<&#original_ty, crate::resource::NotRequested> {
                        self.#ident.as_ref().ok_or(crate::resource::NotRequested(#field_name))
                    }
                }
            };
            getters.push(getter);

            // Wrap the field type in an outer Option
            field.ty = syn::parse_quote!(Option<#original_ty>);

            // Skip serializing when the outer Option is None
            field
                .attrs
                .push(syn::parse_quote!(#[serde(skip_serializing_if = "Option::is_none")]));

            // Only mark non-nullable if the original field was NOT itself an Option
            if !was_option {
                field.attrs.push(syn::parse_quote!(#[schema(nullable = false)]));
            }
        }
    }

    let struct_ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let expanded = quote! {
        #input

        impl #impl_generics #struct_ident #ty_generics #where_clause {
            #(#getters)*
        }
    };

    TokenStream::from(expanded)
}
