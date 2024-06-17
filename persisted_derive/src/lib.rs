use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

/// Derive macro for [PersistedKey](persisted::PersistedKey)
#[proc_macro_derive(PersistedKey, attributes(persisted))]
pub fn persisted_key_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    // Load type from #[persisted(...)] attribute
    let attr = input
        .attrs
        .into_iter()
        .find_map(|attr| {
            let segment = attr.meta.path().segments.first()?;
            if segment.ident == "persisted" {
                Some(attr)
            } else {
                None
            }
        })
        .expect(
            "`PersistedKey` derive requires `#[persisted(<type>)]` attribute \
            to define value type",
        );
    let attr_tokens: TokenStream =
        attr.meta.require_list().unwrap().tokens.clone().into();
    let value_type = parse_macro_input!(attr_tokens as syn::Type);

    quote! {
        #[automatically_derived]
        impl persisted::PersistedKey for #name {
            type Value = #value_type;

            fn type_name() -> &'static str {
                std::any::type_name::<Self>()
            }
        }
    }
    .into()
}
