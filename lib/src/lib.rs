//! Minimalistic testing framework for generating tests for a given set of test cases
//! and decorating them to add retries, timeouts, sequential test processing etc. In other words,
//! the framework implements:
//!
//! - Parameterized tests of reasonably low cardinality for the standard Rust test runner
//! - Fully code-based, composable and extensible test decorators.
//!
//! # Overview
//!
//! ## Test cases
//!
//! [`test_casing`](macro@test_casing) attribute macro wraps a free-standing function
//! with one or more arguments and transforms it into a collection of test cases.
//! The arguments to the function are supplied by an iterator (more precisely,
//! an expression implementing [`IntoIterator`]).
//!
//! For convenience, there is [`TestCases`], a lazy iterator wrapper that allows constructing
//! test cases which cannot be constructed in compile time (e.g., ones requiring access to heap).
//! [`TestCases`] can be instantiated using the [`cases!`] macro.
//!
//! Since a separate test wrapper is generated for each case, their number should be
//! reasonably low (roughly speaking, no more than 20).
//! Isolating each test case makes most sense if the cases involve some heavy lifting
//! (spinning up a runtime, logging considerable amount of information, etc.).
//!
//! ## Test decorators
//!
//! [`decorate`] attribute macro can be placed on a test function to add generic functionality,
//! such as retries, timeouts or running tests in a sequence.
//!
//! The [`decorators`] module defines some basic decorators and the
//! [`DecorateTest`](decorators::DecorateTest) trait allowing to define custom decorators.
//! Test decorators support async tests, tests returning `Result`s and test cases; see
//! the module docs for more details.
//!
//! # Test cases structure
//!
//! The generated test cases are placed in a module with the same name as the target function
//! near the function.
//! This allows specifying the (potentially qualified) function name to restrict the test scope.
//!
//! If the [`nightly` crate feature](#nightly) is not enabled, names of particular test cases
//! are not descriptive; they have the `case_NN` format, where `NN` is the 0-based case index.
//! The values of arguments provided to the test are printed to the standard output
//! at the test start. (The standard output is captured and thus may be not visible
//! unless the `--nocapture` option is specified in the `cargo test` command.)
//!
//! If the `nightly` feature *is* enabled, the names are more descriptive, containing [`Debug`]
//! presentation of all args together with their names. Here's an excerpt from the integration
//! tests for this crate:
//!
//! ```text
//! test number_can_be_converted_to_string::case_1 [number = 3, expected = "3"] ... ok
//! test number_can_be_converted_to_string::case_2 [number = 5, expected = "5"] ... ok
//! test numbers_are_large::case_0 [number = 2] ... ignored, testing that `#[ignore]` attr works
//! test numbers_are_large::case_1 [number = 3] ... ignored, testing that `#[ignore]` attr works
//! test string_conversion_fail::case_0 [bogus_str = "not a number"] - should panic ... ok
//! test string_conversion_fail::case_1 [bogus_str = "-"] - should panic ... ok
//! test string_conversion_fail::case_2 [bogus_str = ""] - should panic ... ok
//! ```
//!
//! The names are fully considered when filtering tests, meaning that it's possible to run
//! particular cases using a filter like `cargo test 'number = 5'`.
//!
//! # Alternatives and similar tools
//!
//! - The approach to test casing from this crate can be reproduced with some amount of copy-pasting
//!   by manually feeding necessary inputs to a common parametric testing function.
//!   Optionally, these tests may be collected in a module for better structuring.
//!   The main downside of this approach is the amount of copy-pasting.
//! - Alternatively, multiple test cases may be run in a single `#[test]` (e.g., in a loop).
//!   This is fine for the large amount of small cases (e.g., mini-fuzzing), but may have downsides
//!   such as overflowing or overlapping logs and increased test runtimes.
//! - The [`test-case`] crate uses a similar approach to test case structuring, but differs
//!   in how test case inputs are specified. Subjectively, the approach used by this crate
//!   is more extensible and easier to read.
//! - [Property testing] / [`quickcheck`]-like frameworks provide much more exhaustive approach
//!   to parameterized testing, but they require significantly more setup effort.
//! - [`rstest`] supports test casing and some of the test decorators (e.g., timeouts).
//! - [`nextest`] is an alternative test runner that supports most of the test decorators
//!   defined in the [`decorators`] module. It does not use code-based decorator config and
//!   does not allow for custom decorator.
//!
//! [`test-case`]: https://docs.rs/test-case/
//! [Property testing]: https://docs.rs/proptest/
//! [`quickcheck`]: https://docs.rs/quickcheck/
//! [`rstest`]: https://crates.io/crates/rstest
//! [`nextest`]: https://nexte.st/
//!
//! # Crate features
//!
//! ## `nightly`
//!
//! *(Off by default)*
//!
//! Uses [custom test frameworks] APIs together with a generous spicing of hacks
//! to include arguments in the names of the generated tests (see an excerpt above
//! for an illustration). `test_casing` actually does not require a custom test runner,
//! but rather hacks into the standard one; thus, the generated test cases can run alongside with
//! ordinary / non-parameterized tests.
//!
//! Requires a nightly Rust toolchain and specifying `#![feature(test, custom_test_frameworks)]`
//! in the using crate. Because `custom_test_frameworks` APIs may change between toolchain releases,
//! the feature may break. See [the CI config] for the nightly toolchain version the crate
//! is tested against.
//!
//! [custom test frameworks]: https://github.com/rust-lang/rust/issues/50297
//! [the CI config]: https://github.com/slowli/test-casing/blob/main/.github/workflows/ci.yml

#![cfg_attr(feature = "nightly", feature(custom_test_frameworks, test))]
// Documentation settings
#![doc(html_root_url = "https://docs.rs/test-casing/0.1.2")]
// Linter settings
#![warn(missing_debug_implementations, missing_docs, bare_trait_objects)]
#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::must_use_candidate, clippy::module_name_repetitions)]

/// Wraps a tested function to add retries, timeouts etc.
///
/// # Inputs
///
/// This attribute must be placed on a test function (i.e., one decorated with `#[test]`,
/// `#[tokio::test]`, etc.). The attribute must be invoked with a comma-separated list
/// of one or more [test decorators](decorators::DecorateTest). Each decorator must
/// be a constant expression (i.e., it should be usable as a definition of a `static` variable).
///
/// # Examples
///
/// ## Basic usage
///
/// ```
/// use test_casing::{decorate, decorators::Timeout};
///
/// #[test]
/// # fn eat_test_attribute() {}
/// #[decorate(Timeout::secs(1))]
/// fn test_with_timeout() {
///     // test logic
/// }
/// ```
///
/// ## Tests returning `Result`s
///
/// Decorators can be used on tests returning `Result`s, too:
///
/// ```
/// use test_casing::{decorate, decorators::{Retry, Timeout}};
/// use std::error::Error;
///
/// #[test]
/// # fn eat_test_attribute() {}
/// #[decorate(Timeout::millis(200), Retry::times(2))]
/// // ^ Decorators are applied in the order of their mention. In this case,
/// // if the test times out, errors or panics, it will be retried up to 2 times.
/// fn test_with_retries() -> Result<(), Box<dyn Error + Send>> {
///     // test logic
/// #   Ok(())
/// }
/// ```
///
/// ## Multiple `decorate` attributes
///
/// Multiple `decorate` attributes are allowed. Thus, the test above is equivalent to
///
/// ```
/// # use test_casing::{decorate, decorators::{Retry, Timeout}};
/// # use std::error::Error;
/// #[test]
/// # fn eat_test_attribute() {}
/// #[decorate(Timeout::millis(200))]
/// #[decorate(Retry::times(2))]
/// fn test_with_retries() -> Result<(), Box<dyn Error + Send>> {
///     // test logic
/// #   Ok(())
/// }
/// ```
///
/// ## Async tests
///
/// Decorators work on async tests as well, as long as the `decorate` macro is applied after
/// the test macro:
///
/// ```
/// # use test_casing::{decorate, decorators::Retry};
/// #[async_std::test]
/// #[decorate(Retry::times(3))]
/// async fn async_test() {
///     // test logic
/// }
/// ```
///
/// ## Composability and reuse
///
/// Decorators can be extracted to a `const`ant or a `static` for readability, composability
/// and/or reuse:
///
/// ```
/// # use test_casing::{decorate, decorators::*};
/// # use std::time::Duration;
/// const RETRY: RetryErrors<String> = Retry::times(2)
///     .with_delay(Duration::from_secs(1))
///     .on_error(|s| s.contains("oops"));
///
/// static SEQUENCE: Sequence = Sequence::new().abort_on_failure();
///
/// #[test]
/// # fn eat_test_attribute() {}
/// #[decorate(RETRY, &SEQUENCE)]
/// fn test_with_error_retries() -> Result<(), String> {
///     // test logic
/// #   Ok(())
/// }
///
/// #[test]
/// # fn eat_test_attribute2() {}
/// #[decorate(&SEQUENCE)]
/// fn other_test() {
///     // test logic
/// }
/// ```
///
/// ## Use with `test_casing`
///
/// When used together with the [`test_casing`](macro@test_casing) macro, the decorators will apply
/// to each generated case.
///
/// ```
/// use test_casing::{decorate, test_casing, decorators::Timeout};
///
/// #[test_casing(3, [3, 5, 42])]
/// #[decorate(Timeout::secs(1))]
/// fn parameterized_test_with_timeout(input: u64) {
///     // test logic
/// }
/// ```
pub use test_casing_macro::decorate;

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
/// ## Basic usage
///
/// `test_casing` macro accepts 2 args: number of cases and the iterator expression.
/// The latter can be any valid Rust expression.
///
/// ```
/// # use test_casing::test_casing;
/// #[test_casing(5, 0..5)]
/// // #[test] attribute is optional and is added automatically
/// // provided that the test function is not `async`.
/// fn number_is_small(number: i32) {
///     assert!(number < 10);
/// }
/// ```
///
/// Functions returning `Result`s are supported as well.
///
/// ```
/// # use test_casing::test_casing;
/// use std::error::Error;
///
/// #[test_casing(3, ["0", "42", "-3"])]
/// fn parsing_numbers(s: &str) -> Result<(), Box<dyn Error>> {
///     let number: i32 = s.parse()?;
///     assert!(number.abs() < 100);
///     Ok(())
/// }
/// ```
///
/// The function on which the `test_casing` attribute is placed can be accessed from other code
/// (e.g., for more tests):
///
/// ```
/// # use test_casing::test_casing;
/// # use std::error::Error;
/// #[test_casing(3, ["0", "42", "-3"])]
/// fn parsing_numbers(s: &str) -> Result<(), Box<dyn Error>> {
///     // snipped...
/// #   Ok(())
/// }
///
/// #[test]
/// fn parsing_number_error() {
///     assert!(parsing_numbers("?").is_err());
/// }
/// ```
///
/// ## Case expressions
///
/// Case expressions can be extracted to a constant for reuse or better code structuring.
///
/// ```
/// # use test_casing::{cases, test_casing, TestCases};
/// const CASES: TestCases<(String, i32)> = cases! {
///     [0, 42, -3].map(|i| (i.to_string(), i))
/// };
///
/// #[test_casing(3, CASES)]
/// fn parsing_numbers(s: String, expected: i32) {
///     let parsed: i32 = s.parse().unwrap();
///     assert_eq!(parsed, expected);
/// }
/// ```
///
/// This example also shows that semantics of args is up to the writer; some of the args may be
/// expected values, etc.
///
/// ## Cartesian product
///
/// One of possible case expressions is a [`Product`]; it can be used to generate test cases
/// as a Cartesian product of the expressions for separate args.
///
/// ```
/// # use test_casing::{test_casing, Product};
/// #[test_casing(6, Product((0_usize..3, ["foo", "bar"])))]
/// fn numbers_and_strings(number: usize, s: &str) {
///     assert!(s.len() <= number);
/// }
/// ```
///
/// ## Reference args
///
/// It is possible to go from a generated argument to its reference by adding
/// a `#[map(ref)]` attribute on the argument. The attribute may optionally specify
/// a path to the transform function from the reference to the desired type
/// (similar to transform specifications in the [`serde`](https://docs.rs/serde/) attr).
///
/// ```
/// # use test_casing::{cases, test_casing, TestCases};
/// const CASES: TestCases<(String, i32)> = cases! {
///     [0, 42, -3].map(|i| (i.to_string(), i))
/// };
///
/// #[test_casing(3, CASES)]
/// fn parsing_numbers(#[map(ref)] s: &str, expected: i32) {
///     // Snipped...
/// }
///
/// #[test_casing(3, CASES)]
/// fn parsing_numbers_too(
///     #[map(ref = String::as_str)] s: &str,
///     expected: i32,
/// ) {
///     // Snipped...
/// }
/// ```
///
/// ## `ignore` and `should_panic` attributes
///
/// `ignore` or `should_panic` attributes can be specified below the `test_casing` attribute.
/// They will apply to all generated tests.
///
/// ```
/// # use test_casing::test_casing;
/// #[test_casing(3, ["not", "implemented", "yet"])]
/// #[ignore = "Promise this will work sometime"]
/// fn future_test(s: &str) {
///     unimplemented!()
/// }
///
/// #[test_casing(3, ["not a number", "-", ""])]
/// #[should_panic(expected = "ParseIntError")]
/// fn string_conversion_fail(bogus_str: &str) {
///     bogus_str.parse::<i32>().unwrap();
/// }
/// ```
///
/// ## Async tests
///
/// `test_casing` supports all kinds of async test wrappers, such as `async_std::test`,
/// `tokio::test`, `actix::test` etc. The corresponding attribute just needs to be specified
/// *below* the `test_casing` attribute.
///
/// ```
/// # use test_casing::test_casing;
/// # use std::error::Error;
/// #[test_casing(3, ["0", "42", "-3"])]
/// #[async_std::test]
/// async fn parsing_numbers(s: &str) -> Result<(), Box<dyn Error>> {
///     assert!(s.parse::<i32>()?.abs() < 100);
///     Ok(())
/// }
/// ```
pub use test_casing_macro::test_casing;

pub mod decorators;
#[cfg(feature = "nightly")]
#[doc(hidden)] // used by the `#[test_casing]` macro; logically private
pub mod nightly;
mod test_casing;

pub use crate::test_casing::{case, ArgNames, Product, ProductIter, TestCases};
