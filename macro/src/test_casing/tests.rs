//! Unit tests for the `test_casing` macro.

use assert_matches::assert_matches;

#[cfg(feature = "nightly")]
use super::nightly::AttrValue;
use super::*;

#[test]
fn parsing_case_attrs() {
    let attr = quote!(3, ["test", "this", "str"]);
    let attrs = CaseAttrs::parse(attr).unwrap();
    assert_eq!(attrs.count, 3);
    assert_eq!(attrs.expr, syn::parse_quote!(["test", "this", "str"]));
}

#[test]
fn parsing_map_attrs() {
    let attr: Attribute = syn::parse_quote!(#[map(ref)]);
    let attr = attr.parse_args::<MapAttrs>().unwrap();
    assert!(attr.path.is_none());

    let attr: Attribute = syn::parse_quote!(#[map(ref = String::as_str)]);
    let attr = attr.parse_args::<MapAttrs>().unwrap();
    let expected: Path = syn::parse_quote!(String::as_str);
    assert_eq!(attr.path.unwrap(), expected);
}

#[test]
fn processing_map_attr_without_path() {
    let attr = MapAttrs { path: None };
    let ident: Ident = syn::parse_quote!(test);
    let mapped = attr.map_arg(&ident);
    let mapped: Expr = syn::parse_quote!(#mapped);
    let expected: Expr = syn::parse_quote!(&test);
    assert_eq!(mapped, expected);
}

#[test]
fn processing_map_attr_with_path() {
    let attr = MapAttrs {
        path: Some(syn::parse_quote!(String::as_str)),
    };
    let ident: Ident = syn::parse_quote!(test);
    let mapped = attr.map_arg(&ident);
    let mapped: Expr = syn::parse_quote!(#mapped);
    let expected: Expr = syn::parse_quote!(String::as_str(&test));
    assert_eq!(mapped, expected);
}

#[test]
fn initializing_fn_wrapper() {
    let attrs = CaseAttrs {
        count: 2,
        expr: syn::parse_quote!(CASES),
    };
    let mut function: ItemFn = syn::parse_quote! {
        #[allow(unused)]
        #[should_panic = "oops"]
        fn tested_fn(number: u32, #[map(ref)] s: &str) {}
    };

    let wrapper = FunctionWrapper::new(attrs, &mut function).unwrap();
    assert_eq!(wrapper.name, "tested_fn");
    assert_matches!(
        wrapper.arg_mappings.as_slice(),
        [None, Some(MapAttrs { path: None })]
    );

    #[cfg(feature = "nightly")]
    {
        assert!(wrapper.fn_attrs.is_empty());
        let nightly_data = wrapper.nightly;
        assert_matches!(nightly_data.should_panic.unwrap(), AttrValue::Str(_));
        assert!(nightly_data.ignore.is_none());
    }
    #[cfg(not(feature = "nightly"))]
    {
        assert_eq!(wrapper.fn_attrs.len(), 2);
        assert_eq!(
            wrapper.fn_attrs[0].path().segments.last().unwrap().ident,
            "test"
        );
        assert!(wrapper.fn_attrs[1].path().is_ident("should_panic"));
    }

    let expected: ItemFn = syn::parse_quote! {
        #[allow(unused)]
        fn tested_fn(number: u32, s: &str) {}
    };
    assert_eq!(function, expected, "{}", quote!(#function));
}

fn create_wrapper() -> FunctionWrapper {
    let attrs = CaseAttrs {
        count: 2,
        expr: syn::parse_quote!(CASES),
    };
    let mut function: ItemFn = syn::parse_quote! {
        fn tested_fn(number: u32, #[map(ref)] s: &str) {}
    };

    FunctionWrapper::new(attrs, &mut function).unwrap()
}

#[test]
fn computing_arg_names() {
    let wrapper = create_wrapper();
    let arg_names: Vec<_> = wrapper.arg_names().collect();
    assert_eq!(arg_names, ["number", "s"]);
}

#[test]
fn computing_case_bindings() {
    let wrapper = create_wrapper();
    let (arg_idents, case_args) = wrapper.case_binding();
    let case_binding = FunctionWrapper::group_idents(&arg_idents);
    let case_binding: Pat = syn::parse_quote!(#case_binding);
    let expected: Pat = syn::parse_quote!((__case_arg0, __case_arg1,));
    assert_eq!(case_binding, expected, "{}", quote!(#case_binding));

    let case_args: Expr = syn::parse_quote!((#case_args));
    let expected: Expr = syn::parse_quote!((__case_arg0, &__case_arg1,));
    assert_eq!(case_args, expected, "{}", quote!(#case_args));
}

#[cfg(feature = "nightly")]
#[test]
fn generating_case() {
    let wrapper = create_wrapper();
    let case_name: Ident = syn::parse_quote!(case0);
    let case_fn = wrapper.case_fn(0, &case_name);
    let case_fn: ItemFn = syn::parse_quote!(#case_fn);

    let expected: ItemFn = syn::parse_quote! {
        fn case0() {
            let (__case_arg0, __case_arg1,) = test_casing::case(CASES, 0usize);
            tested_fn(__case_arg0, &__case_arg1,);
        }
    };
    assert_eq!(case_fn, expected, "{}", quote!(#case_fn));
}

#[cfg(not(feature = "nightly"))]
#[test]
fn generating_case() {
    let wrapper = create_wrapper();
    let case_name: Ident = syn::parse_quote!(case0);
    let case_fn = wrapper.case_fn(0, &case_name);
    let case_fn: ItemFn = syn::parse_quote!(#case_fn);

    let expected: ItemFn = syn::parse_quote! {
        #[::core::prelude::v1::test]
        fn case0() {
            let __case = test_casing::case(CASES, 0usize);
            println!(
                "Testing case #{}: {}",
                0usize,
                test_casing::ArgNames::print_with_args(__ARG_NAMES, &__case)
            );
            let (__case_arg0, __case_arg1,) = __case;
            tested_fn(__case_arg0, &__case_arg1,);
        }
    };
    assert_eq!(case_fn, expected, "{}", quote!(#case_fn));
}
