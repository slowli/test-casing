//! Nightly-specific types and functionality.

use std::fmt;

use quote::{quote, ToTokens};
use syn::{parse::Error as SynError, Attribute, Expr, ExprLit, Lit, Meta, MetaList, MetaNameValue};

pub(crate) enum AttrValue {
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
    fn new(attr: &Attribute, expected_field: Option<&str>) -> syn::Result<Self> {
        match &attr.meta {
            Meta::Path(_) => Ok(Self::Empty),
            Meta::NameValue(MetaNameValue { value, .. }) => {
                if let Expr::Lit(ExprLit {
                    lit: Lit::Str(str), ..
                }) = value
                {
                    Ok(Self::Str(str.clone()))
                } else {
                    let message = "unrecognized attribute value; should be a string literal";
                    Err(SynError::new_spanned(attr, message))
                }
            }
            Meta::List(list) => {
                if let Some(expected_field) = expected_field {
                    Self::from_list(list, expected_field)
                } else {
                    let name = attr.meta.path().get_ident().unwrap();
                    let message = format!(
                        "unrecognized attribute shape; should have `#[{name}] or \
                        `#[{name} = \"value\"]` form"
                    );
                    Err(SynError::new_spanned(attr, message))
                }
            }
        }
    }

    fn from_list(list: &MetaList, expected_field: &str) -> syn::Result<Self> {
        let mut len = 0;
        let mut value = None;
        list.parse_nested_meta(|nested| {
            len += 1;
            if !nested.path.is_ident(expected_field) {
                let message =
                    format!("attribute should have a single field `{expected_field} = \"value\"`");
                return Err(nested.error(message));
            }
            value = Some(nested.value()?.parse::<syn::LitStr>()?);
            Ok(())
        })?;

        if len != 1 {
            let message =
                format!("attribute should have a single field `{expected_field} = \"value\"`");
            return Err(SynError::new_spanned(list, message));
        }
        let value = value.unwrap();
        Ok(Self::Str(value))
    }
}

#[derive(Debug)]
pub(crate) struct NightlyData {
    pub ignore: Option<AttrValue>,
    pub should_panic: Option<AttrValue>,
}

impl NightlyData {
    pub(crate) fn from_attrs(attrs: &mut Vec<Attribute>) -> syn::Result<Self> {
        let mut ignore = None;
        let mut should_panic = None;
        let mut indices_to_remove = vec![];
        for (i, attr) in attrs.iter().enumerate() {
            if attr.path().is_ident("ignore") {
                ignore = Some(AttrValue::new(attr, None)?);
                indices_to_remove.push(i);
            } else if attr.path().is_ident("should_panic") {
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

    pub(crate) fn macro_args(&self) -> impl ToTokens {
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

#[cfg(test)]
mod tests {
    use assert_matches::assert_matches;

    use super::*;

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
}
