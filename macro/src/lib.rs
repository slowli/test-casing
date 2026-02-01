//! Procedural macros for the [`test-casing`] crate.
//!
//! - The `test_casing` macro from this crate flattens parameterized tests into a set of test cases.
//! - The `decorate` macro wraps a tested function to add retries, timeouts etc.
//!
//! See the [`test-casing`] crate docs for macro documentation and examples of usage.
//!
//! [`test-casing`]: https://docs.rs/test-casing/

// Documentation settings
#![doc(html_root_url = "https://docs.rs/test-casing-macro/0.2.0-beta.1")]
#![allow(missing_docs)] // The macros are documented in the main crate

extern crate proc_macro;

use proc_macro::TokenStream;

mod decorate;
mod test_casing;

use crate::{decorate::impl_decorate, test_casing::impl_test_casing};

#[proc_macro_attribute]
pub fn test_casing(attr: TokenStream, item: TokenStream) -> TokenStream {
    match impl_test_casing(attr, item) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.into_compile_error().into(),
    }
}

#[proc_macro_attribute]
pub fn decorate(attr: TokenStream, item: TokenStream) -> TokenStream {
    match impl_decorate(attr, item) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.into_compile_error().into(),
    }
}
