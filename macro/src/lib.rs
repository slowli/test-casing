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

mod test_casing;

use crate::test_casing::impl_test_casing;

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
    impl_test_casing(attr, item).into()
}
