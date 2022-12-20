//! Macros for the [`test-casing`] crate.

extern crate proc_macro;

use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{
    ext::IdentExt,
    parse::{Error as SynError, Parse, ParseStream},
    spanned::Spanned,
    Attribute, Expr, FnArg, Ident, Item, ItemFn, Lit, LitInt, Meta, MetaList, MetaNameValue,
    NestedMeta, Pat, PatType, Path, ReturnType, Signature, Token,
};

use std::{fmt, mem};

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
    fn parse(attr: TokenStream) -> syn::Result<Self> {
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

        let syntax: CaseAttrsSyntax = syn::parse(attr)?;
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

enum AttrValue {
    Empty,
    Str(syn::LitStr),
}

impl fmt::Debug for AttrValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.debug_tuple("Empty").finish(),
            Self::Str(s) => formatter.debug_tuple("Str").field(&s.value()).finish(),
        }
    }
}

impl AttrValue {
    fn new(attr: &Attribute, inner_attr: Option<&str>) -> syn::Result<Self> {
        match attr.parse_meta()? {
            Meta::Path(_) => Ok(Self::Empty),
            Meta::NameValue(MetaNameValue { lit, .. }) => {
                if let Lit::Str(str) = lit {
                    Ok(Self::Str(str))
                } else {
                    let message = "unrecognized attribute value; should be a string literal";
                    Err(SynError::new_spanned(attr, message))
                }
            }
            Meta::List(list) => {
                if let Some(inner_attr) = inner_attr {
                    Self::from_list(list, inner_attr)
                } else {
                    let name = attr.path.get_ident().unwrap();
                    let message = format!(
                        "unrecognized attribute shape; should have `#[{name}] or \
                        `#[{name} = \"value\"]` form"
                    );
                    Err(SynError::new_spanned(attr, message))
                }
            }
        }
    }

    fn from_list(list: MetaList, inner_attr: &str) -> syn::Result<Self> {
        let list = list.nested;
        if list.len() != 1 {
            let message =
                format!("attribute should have a single field `{inner_attr} = \"value\"`");
            return Err(SynError::new_spanned(&list, message));
        }
        let NestedMeta::Meta(Meta::NameValue(nv)) = list.first().unwrap() else {
            let message =
                format!("attribute should have a single field `{inner_attr} = \"value\"`");
            return Err(SynError::new_spanned(&list, message));
        };

        if !nv.path.is_ident(inner_attr) {
            let message =
                format!("attribute should have a single field `{inner_attr} = \"value\"`");
            return Err(SynError::new_spanned(&list, message));
        }

        if let Lit::Str(str) = &nv.lit {
            Ok(Self::Str(str.clone()))
        } else {
            let message = "unrecognized attribute value; should be a string literal";
            Err(SynError::new_spanned(nv, message))
        }
    }
}

#[derive(Debug)]
struct NightlyData {
    ignore: Option<AttrValue>,
    should_panic: Option<AttrValue>,
}

impl NightlyData {
    fn from_attrs(attrs: &mut Vec<Attribute>) -> syn::Result<Self> {
        let mut ignore = None;
        let mut should_panic = None;
        let mut indices_to_remove = vec![];
        for (i, attr) in attrs.iter().enumerate() {
            if attr.path.is_ident("ignore") {
                ignore = Some(AttrValue::new(attr, None)?);
                indices_to_remove.push(i);
            } else if attr.path.is_ident("should_panic") {
                should_panic = Some(AttrValue::new(attr, Some("expected"))?);
                indices_to_remove.push(i);
            }
        }

        for i in indices_to_remove.into_iter().rev() {
            attrs.remove(i);
        }
        Ok(Self {
            ignore,
            should_panic,
        })
    }

    fn macro_args(&self) -> impl ToTokens {
        let option = quote!(::core::option::Option);
        let ignore = self.ignore.as_ref().map(|ignore| match ignore {
            AttrValue::Empty => quote!(ignore: #option::None,),
            AttrValue::Str(s) => quote!(ignore: #option::Some(#s),),
        });
        let should_panic = self.should_panic.as_ref().map(|panic| match panic {
            AttrValue::Empty => quote!(panic_message: #option::None,),
            AttrValue::Str(s) => quote!(panic_message: #option::Some(#s),),
        });
        quote! { #ignore #should_panic }
    }
}

struct FunctionWrapper {
    nightly: Option<NightlyData>,
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
            .field("nightly", &self.nightly)
            .field("attrs", &self.attrs)
            .field("name", &self.name)
            .field("fn_attrs_len", &self.fn_attrs.len())
            .finish_non_exhaustive()
    }
}

impl FunctionWrapper {
    fn new(attrs: CaseAttrs, function: &mut ItemFn) -> syn::Result<Self> {
        if function.sig.inputs.is_empty() {
            let message = "tested function must have at least one input";
            return Err(SynError::new_spanned(&function.sig, message));
        }
        let mappings = function.sig.inputs.iter_mut().map(|arg| {
            let attrs = match arg {
                FnArg::Receiver(receiver) => &mut receiver.attrs,
                FnArg::Typed(typed) => &mut typed.attrs,
            };
            let map_attr = attrs
                .iter()
                .enumerate()
                .find(|(_, attr)| attr.path.is_ident("map"));
            let Some((idx, map_attr)) = map_attr else {
                return Ok(None);
            };
            let map_attr = map_attr.parse_args::<MapAttrs>()?;
            attrs.remove(idx);
            Ok(Some(map_attr))
        });
        let mappings: syn::Result<Vec<_>> = mappings.collect();
        let mappings = mappings?;

        let mut fn_attrs = mem::take(&mut function.attrs);
        let test_attr_position = fn_attrs.iter().position(|attr| attr.path.is_ident("test"));
        if cfg!(feature = "nightly") {
            if let Some(position) = test_attr_position {
                fn_attrs.remove(position);
            }
        } else if test_attr_position.is_none() && function.sig.asyncness.is_none() {
            let test_attr = syn::parse_quote!(#[::core::prelude::v1::test]);
            fn_attrs.insert(0, test_attr);
        }

        Ok(Self {
            nightly: if cfg!(feature = "nightly") {
                Some(NightlyData::from_attrs(&mut fn_attrs)?)
            } else {
                None
            },
            name: function.sig.ident.clone(),
            attrs,
            fn_attrs,
            fn_sig: function.sig.clone(),
            arg_mappings: mappings,
        })
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
                FnArg::Typed(PatType { pat, .. }) => match pat.as_ref() {
                    Pat::Ident(ident) => ident.ident.to_string(),
                    _ => format!("(arg {i})"),
                },
            });
        quote! {
            const __ARG_NAMES: [&'static str; #arg_count] = [#(#arg_names,)*];
        }
    }

    fn wrapper(&self) -> impl ToTokens {
        let name = &self.name;
        let arg_names = self.arg_names();
        let index_width = (self.attrs.count - 1).to_string().len();
        let cases = (0..self.attrs.count).map(|i| self.case(i, index_width));
        quote! {
            #[cfg(test)]
            mod #name {
                use super::*;
                #arg_names
                #(#cases)*
            }
        }
    }

    fn declare_test_case(&self, index: usize, test_fn_name: &Ident) -> impl ToTokens {
        let cr = quote!(test_casing);
        let cases_expr = &self.attrs.expr;
        let test_case_name = format!("__TEST_CASE_{index}");
        let test_case_name = Ident::new(&test_case_name, self.name.span());
        let additional_args = self.nightly.as_ref().unwrap().macro_args();

        quote! {
            #[::core::prelude::v1::test_case]
            static #test_case_name: #cr::nightly::LazyTestCase = #cr::declare_test_case!(
                base_name: ::core::module_path!(),
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

        let case_fn = self.case_fn(index, &case_name, self.nightly.is_some());
        if self.nightly.is_some() {
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
        } else {
            case_fn
        }
    }

    fn case_fn(&self, index: usize, case_name: &Ident, nightly: bool) -> proc_macro2::TokenStream {
        let cr = quote!(test_casing);
        let name = &self.name;
        let attrs = &self.fn_attrs;

        let maybe_async = &self.fn_sig.asyncness;
        let maybe_await = maybe_async.as_ref().map(|_| quote!(.await));
        let ret = &self.fn_sig.output;
        let maybe_semicolon = match ret {
            ReturnType::Default => Some(quote!(;)),
            _ => None,
        };
        let cases_expr = &self.attrs.expr;

        let (case_binding, case_args) = if self.fn_sig.inputs.len() == 1 {
            let arg = self.fn_sig.inputs.first().unwrap().span();
            let arg = Ident::new("case_arg", arg.span());
            let mapped_arg = self.arg_mappings[0]
                .as_ref()
                .map_or_else(|| quote!(#arg), |mapping| mapping.map_arg(&arg));
            (quote!(#arg), mapped_arg)
        } else {
            let args = self.fn_sig.inputs.iter().enumerate();
            let args = args.map(|(idx, arg)| Ident::new(&format!("case_arg{idx}"), arg.span()));
            let binding_args = args.clone();
            let case_binding = quote!((#(#binding_args,)*));

            let args = args.zip(&self.arg_mappings).map(|(arg, mapping)| {
                mapping
                    .as_ref()
                    .map_or_else(|| quote!(#arg), |mapping| mapping.map_arg(&arg))
            });
            let case_args = quote!(#(#args,)*);
            (case_binding, case_args)
        };

        let case_assignment = if nightly {
            quote! {
                let #case_binding = #cr::case(cases, #index);
            }
        } else {
            quote! {
                let case = #cr::case(cases, #index);
                println!(
                    "Testing case #{}: {}",
                    #index,
                    #cr::ArgNames::print_with_args(__ARG_NAMES, &case)
                );
                let #case_binding = case;
            }
        };

        quote! {
            #(#attrs)*
            #maybe_async fn #case_name() #ret {
                let cases = #cases_expr;
                #case_assignment
                #name(#case_args) #maybe_await #maybe_semicolon
            }
        }
    }
}

#[proc_macro_attribute]
pub fn test_casing(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attrs = match CaseAttrs::parse(attr) {
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
