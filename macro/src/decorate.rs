//! `decorate` proc macro implementation.

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    spanned::Spanned,
    Error as SynError, Expr, Item, ItemFn, ReturnType, Token,
};

use std::fmt;

struct DecorateAttrs {
    decorators: Vec<Expr>,
}

impl fmt::Debug for DecorateAttrs {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DecorateAttrs")
            .field("decorators_len", &self.decorators.len())
            .finish()
    }
}

impl Parse for DecorateAttrs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let decorators = Punctuated::<Expr, Token![,]>::parse_terminated(input)?;
        Ok(Self {
            decorators: decorators.into_iter().collect(),
        })
    }
}

impl DecorateAttrs {
    fn decorate(&self, function: &ItemFn) -> syn::Result<proc_macro2::TokenStream> {
        let ItemFn {
            attrs,
            vis,
            sig,
            block,
        } = function;

        if let Some(asyncness) = &sig.asyncness {
            let message = "Cannot decorate an async function. Make sure that #[decorate] \
                is applied *after* an attribute for the async test, such as #[tokio::test]";
            return Err(SynError::new(asyncness.span(), message));
        }
        if !sig.inputs.is_empty() {
            let message = "Cannot decorate a function with attributes";
            return Err(SynError::new_spanned(&sig.inputs, message));
        }

        let cr = quote!(test_casing);
        let decorators = &self.decorators;
        let ret_value = &sig.output;
        let ret_value_or_void = match &sig.output {
            ReturnType::Default => quote!(()),
            ReturnType::Type(_, ty) => quote!(#ty),
        };
        let maybe_semicolon = if matches!(ret_value, ReturnType::Default) {
            Some(quote!(;))
        } else {
            None
        };

        Ok(quote! {
            #(#attrs)*
            #vis #sig {
                static __DECORATORS: &dyn #cr::DecorateTestFn<#ret_value_or_void> =
                    &(#(#decorators,)*);
                let __test_fn = || #ret_value #block;
                #cr::DecorateTestFn::decorate_and_test_fn(__DECORATORS, __test_fn) #maybe_semicolon
            }
        })
    }
}

pub(crate) fn impl_decorate(
    attr: TokenStream,
    item: TokenStream,
) -> syn::Result<proc_macro2::TokenStream> {
    let attrs: DecorateAttrs = syn::parse(attr)?;
    let item: Item = syn::parse(item)?;
    match item {
        Item::Fn(function) => attrs.decorate(&function),
        item => {
            let message = "Item is not supported; use `#[decorate] on functions";
            Err(SynError::new_spanned(&item, message))
        }
    }
}
