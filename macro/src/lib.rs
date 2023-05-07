//! Procedural macros for the [`test-casing`] crate.
//!
//! The `test_casing` macro from this crate allows generating flattening parameterized tests
//! into a set of test cases.
//!
//! See `test-casing` docs for details and examples of usage.
//!
//! [`test-casing`]: https://docs.rs/test-casing/

// Documentation settings
#![doc(html_root_url = "https://docs.rs/test-casing-macro/0.1.0")]
// Linter settings
#![warn(missing_debug_implementations, missing_docs, bare_trait_objects)]
#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::must_use_candidate, clippy::module_name_repetitions)]

extern crate proc_macro;

use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{
    ext::IdentExt,
    parse::{Error as SynError, Parse, ParseStream},
    spanned::Spanned,
    Attribute, Expr, FnArg, Ident, Item, ItemFn, LitInt, Pat, PatType, Path, ReturnType, Signature,
    Token,
};

use std::{fmt, mem};

#[cfg(feature = "nightly")]
mod nightly;
#[cfg(test)]
mod tests;

#[cfg(feature = "nightly")]
use self::nightly::NightlyData;

struct CaseAttrs {
    count: usize,
    expr: Expr,
}

impl fmt::Debug for CaseAttrs {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CaseAttrs")
            .field("count", &self.count)
            .finish_non_exhaustive()
    }
}

impl CaseAttrs {
    fn parse(attr: proc_macro2::TokenStream) -> syn::Result<Self> {
        struct CaseAttrsSyntax {
            count: LitInt,
            _comma: Token![,],
            expr: Expr,
        }

        impl Parse for CaseAttrsSyntax {
            fn parse(input: ParseStream) -> syn::Result<Self> {
                Ok(Self {
                    count: input.parse()?,
                    _comma: input.parse()?,
                    expr: input.parse()?,
                })
            }
        }

        let syntax: CaseAttrsSyntax = syn::parse2(attr)?;
        let count: usize = syntax.count.base10_parse()?;
        if count == 0 {
            let message = "number of test cases must be positive";
            return Err(SynError::new(syntax.count.span(), message));
        }
        Ok(Self {
            count,
            expr: syntax.expr,
        })
    }
}

struct MapAttrs {
    path: Option<Path>,
}

impl fmt::Debug for MapAttrs {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MapAttrs")
            .field("path", &self.path.as_ref().map(|_| "_"))
            .finish()
    }
}

impl MapAttrs {
    fn map_arg(&self, arg: &Ident) -> proc_macro2::TokenStream {
        if let Some(path) = &self.path {
            quote!(#path(&#arg))
        } else {
            quote!(&#arg)
        }
    }
}

impl Parse for MapAttrs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        struct MapAttrsSyntax {
            base: Ident,
            path_expr: Option<(Token![=], Path)>,
        }

        impl Parse for MapAttrsSyntax {
            fn parse(input: ParseStream) -> syn::Result<Self> {
                Ok(Self {
                    base: input.call(Ident::parse_any)?,
                    path_expr: if input.peek(Token![=]) {
                        Some((input.parse()?, input.parse()?))
                    } else {
                        None
                    },
                })
            }
        }

        let syntax = MapAttrsSyntax::parse(input)?;
        if syntax.base != "ref" {
            let message = "unknown map transform; only `ref` is supported";
            return Err(SynError::new(syntax.base.span(), message));
        }

        Ok(Self {
            path: syntax.path_expr.map(|(_, path)| path),
        })
    }
}

struct FunctionWrapper {
    #[cfg(feature = "nightly")]
    nightly: NightlyData,
    name: Ident,
    attrs: CaseAttrs,
    fn_attrs: Vec<Attribute>,
    fn_sig: Signature,
    arg_mappings: Vec<Option<MapAttrs>>,
}

impl fmt::Debug for FunctionWrapper {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FunctionWrapper")
            .field("attrs", &self.attrs)
            .field("name", &self.name)
            .field("fn_attrs_len", &self.fn_attrs.len())
            .finish_non_exhaustive()
    }
}

impl FunctionWrapper {
    const MAX_ARGS: usize = 7;

    fn new(attrs: CaseAttrs, function: &mut ItemFn) -> syn::Result<Self> {
        if function.sig.inputs.is_empty() {
            let message = "tested function must have at least one arg";
            return Err(SynError::new_spanned(&function.sig, message));
        } else if function.sig.inputs.len() > Self::MAX_ARGS {
            let message = format!(
                "tested function must have no more than {} args",
                Self::MAX_ARGS
            );
            return Err(SynError::new_spanned(&function.sig, message));
        }

        let generic_params = &function.sig.generics.params;
        if !generic_params.is_empty() {
            let message = "generic tested functions are not supported";
            return Err(SynError::new_spanned(generic_params, message));
        }

        let mappings = function.sig.inputs.iter_mut().map(|arg| {
            let attrs = match arg {
                FnArg::Receiver(receiver) => &mut receiver.attrs,
                FnArg::Typed(typed) => &mut typed.attrs,
            };
            let map_attr = attrs
                .iter()
                .enumerate()
                .find(|(_, attr)| attr.path().is_ident("map"));
            let Some((idx, map_attr)) = map_attr else {
                return Ok(None);
            };
            let map_attr = map_attr.parse_args::<MapAttrs>()?;
            attrs.remove(idx);
            Ok(Some(map_attr))
        });
        let mappings: syn::Result<Vec<_>> = mappings.collect();
        let mappings = mappings?;

        let (retained_attrs, mut fn_attrs) = mem::take(&mut function.attrs)
            .into_iter()
            .partition(Self::should_be_retained);
        function.attrs = retained_attrs;
        let test_attr_position = fn_attrs
            .iter()
            .position(|attr| attr.path().is_ident("test"));
        if cfg!(feature = "nightly") {
            if let Some(position) = test_attr_position {
                fn_attrs.remove(position);
            }
        } else if test_attr_position.is_none() && function.sig.asyncness.is_none() {
            let test_attr = syn::parse_quote!(#[::core::prelude::v1::test]);
            fn_attrs.insert(0, test_attr);
        }

        Ok(Self {
            #[cfg(feature = "nightly")]
            nightly: NightlyData::from_attrs(&mut fn_attrs)?,
            name: function.sig.ident.clone(),
            attrs,
            fn_attrs,
            fn_sig: function.sig.clone(),
            arg_mappings: mappings,
        })
    }

    // FIXME: this is extremely hacky. Ideally, we'd want to partition attrs by their location
    //   before / after `#[test_casing]`, but this seems impossible on stable Rust (span locations
    //   are unstable).
    fn should_be_retained(attr: &Attribute) -> bool {
        attr.path().is_ident("allow")
            || attr.path().is_ident("warn")
            || attr.path().is_ident("deny")
            || attr.path().is_ident("forbid")
    }

    fn arg_names(&self) -> impl ToTokens {
        let arg_count = self.fn_sig.inputs.len();
        let arg_names = self
            .fn_sig
            .inputs
            .iter()
            .enumerate()
            .map(|(i, arg)| match arg {
                FnArg::Receiver(_) => String::from("self"),
                FnArg::Typed(PatType { pat, .. }) => {
                    if let Pat::Ident(ident) = pat.as_ref() {
                        ident.ident.to_string()
                    } else {
                        format!("(arg {i})")
                    }
                }
            });
        quote! {
            const __ARG_NAMES: [&'static str; #arg_count] = [#(#arg_names,)*];
        }
    }

    fn test_cases_iter(&self) -> impl ToTokens {
        let cr = quote!(test_casing);
        let name = &self.name;
        let cases_expr = &self.attrs.expr;
        let (case_binding, case_args) = self.case_binding();

        quote! {
            const _: () = {
                #[allow(dead_code)]
                fn __test_cases_iterator() {
                    let #case_binding = #cr::case(#cases_expr, 0);
                    let _ = #name(#case_args);
                }
            };
        }
    }

    fn wrapper(&self) -> impl ToTokens {
        let name = &self.name;
        let test_cases_iter = self.test_cases_iter();
        let arg_names = self.arg_names();
        let index_width = (self.attrs.count - 1).to_string().len();
        let cases = (0..self.attrs.count).map(|i| self.case(i, index_width));

        quote! {
            // Access the iterator to ensure it works even if not building for tests.
            #test_cases_iter

            #[cfg(test)]
            mod #name {
                use super::*;
                #arg_names
                #(#cases)*
            }
        }
    }

    #[cfg(feature = "nightly")]
    fn declare_test_case(&self, index: usize, test_fn_name: &Ident) -> impl ToTokens {
        let cr = quote!(test_casing);
        let cases_expr = &self.attrs.expr;
        let test_case_name = format!("__TEST_CASE_{index}");
        let test_case_name = Ident::new(&test_case_name, self.name.span());
        let additional_args = self.nightly.macro_args();

        let span_start = self.name.span().start();
        let start_line = span_start.line;
        let start_col = span_start.column;
        let span_end = self.name.span().end();
        let end_line = span_end.line;
        let end_col = span_end.column;

        quote! {
            #[::core::prelude::v1::test_case]
            static #test_case_name: #cr::nightly::LazyTestCase = #cr::declare_test_case!(
                base_name: ::core::module_path!(),
                source_file: ::core::file!(),
                start_line: #start_line,
                start_col: #start_col,
                end_line: #end_line,
                end_col: #end_col,
                arg_names: __ARG_NAMES,
                cases: #cases_expr,
                index: #index,
                #additional_args
                testfn: #test_fn_name
            );
        }
    }

    fn case(&self, index: usize, index_width: usize) -> impl ToTokens {
        let case_name = format!("case_{index:0>index_width$}");
        let case_name = Ident::new(&case_name, self.name.span());

        #[cfg(feature = "nightly")]
        {
            let case_fn = self.case_fn(index, &case_name);
            let test_fn_name = format!("__TEST_FN_{index}");
            let test_fn_name = Ident::new(&test_fn_name, self.name.span());
            let ret = &self.fn_sig.output;
            let case_decl = self.declare_test_case(index, &test_fn_name);

            quote! {
                #[allow(unnameable_test_items)]
                // ^ This is a very roundabout way to effectively drop the `#[test]` attribute
                // from the generated code. It should work for all kinds of test macros,
                // such as `async_std::test` or `tokio::test`, without any additional work.
                const #test_fn_name: fn() #ret = {
                    #case_fn
                    #case_name
                };
                #case_decl
            }
        }

        #[cfg(not(feature = "nightly"))]
        self.case_fn(index, &case_name)
    }

    fn case_fn(&self, index: usize, case_name: &Ident) -> proc_macro2::TokenStream {
        let cr = quote!(test_casing);
        let name = &self.name;
        let attrs = &self.fn_attrs;

        let maybe_async = &self.fn_sig.asyncness;
        let maybe_await = maybe_async.as_ref().map(|_| quote!(.await));
        let ret = &self.fn_sig.output;
        let maybe_semicolon = match ret {
            ReturnType::Default => Some(quote!(;)),
            ReturnType::Type { .. } => None,
        };
        let cases_expr = &self.attrs.expr;
        let (case_binding, case_args) = self.case_binding();

        let case_assignment = if cfg!(feature = "nightly") {
            quote! {
                let #case_binding = #cr::case(#cases_expr, #index);
            }
        } else {
            quote! {
                let __case = #cr::case(#cases_expr, #index);
                println!(
                    "Testing case #{}: {}",
                    #index,
                    #cr::ArgNames::print_with_args(__ARG_NAMES, &__case)
                );
                let #case_binding = __case;
            }
        };

        quote! {
            #(#attrs)*
            #maybe_async fn #case_name() #ret {
                #case_assignment
                #name(#case_args) #maybe_await #maybe_semicolon
            }
        }
    }

    /// Returns the binding of args supplied to the test case and potentially mapped args
    /// to provide to the test function.
    fn case_binding(&self) -> (impl ToTokens, impl ToTokens) {
        if self.fn_sig.inputs.len() == 1 {
            let arg = self.fn_sig.inputs.first().unwrap();
            let arg = Ident::new("__case_arg", arg.span());
            let mapped_arg = self.arg_mappings[0]
                .as_ref()
                .map_or_else(|| quote!(#arg), |mapping| mapping.map_arg(&arg));
            (quote!(#arg), mapped_arg)
        } else {
            let args = self.fn_sig.inputs.iter().enumerate();
            let args = args.map(|(idx, arg)| Ident::new(&format!("__case_arg{idx}"), arg.span()));
            let binding_args = args.clone();
            let case_binding = quote!((#(#binding_args,)*));

            let args = args.zip(&self.arg_mappings).map(|(arg, mapping)| {
                mapping
                    .as_ref()
                    .map_or_else(|| quote!(#arg), |mapping| mapping.map_arg(&arg))
            });
            let case_args = quote!(#(#args,)*);
            (case_binding, case_args)
        }
    }
}

/// Flattens a parameterized test into a collection of test cases.
///
/// # Inputs
///
/// This attribute must be placed on a free-standing function with 1..8 arguments.
/// The attribute must be invoked with 2 values:
///
/// 1. Number of test cases, a number literal
/// 2. A *case iterator* expression evaluating to an implementation of [`IntoIterator`]
///   with [`Debug`]gable, `'static` items.
///   If the target function has a single argument, the iterator item type must equal to
///   the argument type. Otherwise, the iterator must return a tuple in which each item
///   corresponds to the argument with the same index.
///
/// A case iterator expression may reference the environment (e.g., it can be a name of a constant).
/// It doesn't need to be a constant expression (e.g., it may allocate in heap). It should
/// return at least the number of items specified as the first attribute argument, and can
/// return more items; these additional items will not be tested.
///
/// [`Debug`]: core::fmt::Debug
///
/// # Mapping arguments
///
/// To support more idiomatic signatures for parameterized test functions, it is possible
/// to *map* from the type returned by the case iterator. The only supported kind of mapping
/// so far is taking a shared reference (i.e., `T` → `&T`). The mapping is enabled by placing
/// the `#[map(ref)]` attribute on the corresponding argument. Optionally, the reference `&T`
/// can be further mapped with a function / method (e.g., `&String` → `&str` with
/// [`String::as_str()`]). This is specified as `#[map(ref = path::to::method)]`, a la
/// `serde` transforms.
///
/// # Examples
///
/// See `test-casing` crate-level docs for the examples of usage.
#[proc_macro_attribute]
pub fn test_casing(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attrs = match CaseAttrs::parse(attr.into()) {
        Ok(attrs) => attrs,
        Err(err) => return err.into_compile_error().into(),
    };
    let tokens = match syn::parse(item) {
        Ok(Item::Fn(mut function)) => FunctionWrapper::new(attrs, &mut function).map(|wrapper| {
            let wrapper = wrapper.wrapper();
            quote!(#function #wrapper)
        }),
        Ok(item) => {
            let message = "Item is not supported; use `#[test_cases] on functions";
            Err(SynError::new_spanned(&item, message))
        }
        Err(err) => return err.into_compile_error().into(),
    };

    match tokens {
        Ok(tokens) => tokens.into(),
        Err(err) => err.into_compile_error().into(),
    }
}
