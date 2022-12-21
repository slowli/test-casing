//! Unit tests for the `test_casing` macro.

use assert_matches::assert_matches;

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
fn extracting_attr_value() {
    let attr: Attribute = syn::parse_quote!(#[ignore]);
    let value = AttrValue::new(&attr, None).unwrap();
    assert_matches!(value, AttrValue::Empty);

    let attr: Attribute = syn::parse_quote!(#[ignore = "TODO"]);
    let value = AttrValue::new(&attr, None).unwrap();
    assert_matches!(value, AttrValue::Str(s) if s.value() == "TODO");

    let attr: Attribute = syn::parse_quote!(#[should_panic = "not available"]);
    let value = AttrValue::new(&attr, Some("expected")).unwrap();
    assert_matches!(value, AttrValue::Str(s) if s.value() == "not available");

    let attr: Attribute = syn::parse_quote!(#[should_panic(expected = "not available")]);
    let value = AttrValue::new(&attr, Some("expected")).unwrap();
    assert_matches!(value, AttrValue::Str(s) if s.value() == "not available");
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
    assert_eq!(wrapper.fn_attrs.len(), 2);
    assert_eq!(
        wrapper.fn_attrs[0].path.segments.last().unwrap().ident,
        "test"
    );
    assert!(wrapper.fn_attrs[1].path.is_ident("should_panic"));

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
    let arg_names = wrapper.arg_names();
    let arg_names: Item = syn::parse_quote!(#arg_names);
    let expected: Item = syn::parse_quote! {
        const __ARG_NAMES: [&'static str; 2usize] = ["number", "s",];
    };
    assert_eq!(arg_names, expected, "{}", quote!(#arg_names));
}

#[test]
fn computing_case_bindings() {
    let wrapper = create_wrapper();
    let (case_binding, case_args) = wrapper.case_binding();
    let case_binding: Pat = syn::parse_quote!(#case_binding);
    let expected: Pat = syn::parse_quote!((__case_arg0, __case_arg1,));
    assert_eq!(case_binding, expected, "{}", quote!(#case_binding));

    let case_args: Expr = syn::parse_quote!((#case_args));
    let expected: Expr = syn::parse_quote!((__case_arg0, &__case_arg1,));
    assert_eq!(case_args, expected, "{}", quote!(#case_args));
}

#[test]
fn generating_case() {
    let mut wrapper = create_wrapper();
    wrapper.nightly = None;
    let case_name: Ident = syn::parse_quote!(case0);
    let case_fn = wrapper.case_fn(0, &case_name, false);
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
