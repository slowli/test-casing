# Parameterized Rust Tests & Test Decorators

[![Build Status](https://github.com/slowli/test-casing/workflows/CI/badge.svg?branch=main)](https://github.com/slowli/test-casing/actions)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%2FApache--2.0-blue)](https://github.com/slowli/test-casing#license)
![rust 1.70+ required](https://img.shields.io/badge/rust-1.70+-blue.svg?label=Required%20Rust)

**Documentation:** [![Docs.rs](https://docs.rs/test-casing/badge.svg)](https://docs.rs/test-casing/)
[![crate docs (main)](https://img.shields.io/badge/main-yellow.svg?label=docs)](https://slowli.github.io/test-casing/test_casing/)

`test-casing` is a minimalistic Rust framework for generating tests for a given set of test cases
and decorating them to add retries, timeouts, sequential test processing etc.
In other words, the framework implements:

- Parameterized tests of reasonably low cardinality for the standard Rust test runner
- Fully code-based, composable and extensible test decorators.

Since a separate test wrapper is generated for each case, their number should be 
reasonably low (roughly speaking, no more than 20).
Isolating each test case makes most sense if the cases involve some heavy lifting
(spinning up a runtime, logging considerable amount of information, etc.).

## Usage

Add this to your `Crate.toml`:

```toml
[dev-dependencies]
test-casing = "0.1.3"
```

### Examples: test cases

```rust
use test_casing::{cases, test_casing, TestCases};
use std::error::Error;

#[test_casing(4, [2, 3, 5, 8])]
fn numeric_test(number: i32) {
    assert!(number < 10);
}

// Cases can be extracted to a constant for better readability.
const CASES: TestCases<(String, i32)> = cases! {
    [2, 3, 5, 8].map(|i| (i.to_string(), i))
};

#[test_casing(4, CASES)]
fn parsing_number(
    #[map(ref)] s: &str,
    // ^ specifies that argument should be borrowed from `String`
    // returned by the `CASES` iterator
    expected: i32,
) -> Result<(), Box<dyn Error>> {
    assert_eq!(s.parse::<i32>()?, expected);
    Ok(())
}
```

Other features include the support of async tests and `ignore` / `should_panic`
attributes (the latter are applied to all generated cases).

```rust
use test_casing::test_casing;

#[test_casing(4, [2, 3, 5, 8])]
#[async_std::test]
// ^ test attribute should be specified below the case spec
async fn test_async(number: i32) {
    assert!(number < 10);
}

#[test_casing(3, ["not", "a", "number"])]
#[should_panic(expected = "ParseIntError")]
fn parsing_number_errors(s: &str) {
    s.parse::<i32>().unwrap();
}
```

### Examples: test decorators

```rust
use test_casing::{
    decorate, test_casing, decorators::{Retry, Sequence, Timeout},
};

#[test]
#[decorate(Retry::times(3), Timeout::secs(3))]
fn test_with_retry_and_timeouts() {
    // Test logic
}

static SEQUENCE: Sequence = Sequence::new().abort_on_failure();

// Execute all test cases sequentially and abort if one of them fails.
#[test_casing(4, [2, 3, 5, 8])]
#[async_std::test]
#[decorate(&SEQUENCE)]
async fn test_async(number: i32) {
    assert!(number < 10);
}
```

See the crate docs for more examples of usage.

### Descriptive test case names

With the help of [custom test frameworks] APIs and a generous spicing of hacks,
the names of generated tests include the values of arguments provided
to the targeted test function if the `nightly` crate feature is enabled.
As the name implies, the feature only works on the nightly Rust.

Here's an excerpt of the output of integration tests in this crate to illustrate:

```text
test cartesian_product::case_6 [number = 5, s = "first"] ... ok
test cartesian_product::case_9 [number = 8, s = "first"] ... ok
test number_can_be_converted_to_string::case_0 [number = 2, expected = "2"] ... ok
test number_can_be_converted_to_string::case_1 [number = 3, expected = "3"] ... ok
test number_can_be_converted_to_string::case_2 [number = 5, expected = "5"] ... ok
test number_can_be_converted_to_string_with_tuple_input::case_0 [(arg 0) = (2, "2")] ... ok
test number_can_be_converted_to_string_with_tuple_input::case_1 [(arg 0) = (3, "3")] ... ok
test number_can_be_converted_to_string_with_tuple_input::case_2 [(arg 0) = (5, "5")] ... ok
test numbers_are_large::case_0 [number = 2] ... ignored, testing that `#[ignore]` attr works
test numbers_are_large::case_1 [number = 3] ... ignored, testing that `#[ignore]` attr works
test string_conversion_fail::case_0 [bogus_str = "not a number"] - should panic ... ok
test string_conversion_fail::case_1 [bogus_str = "-"] - should panic ... ok
test string_conversion_fail::case_2 [bogus_str = ""] - should panic ... ok
test unit_test_detection_works ... ok
```

The arguments are a full-fledged part of test names, meaning that they can be included
into test filters (like `cargo test 'number = 3'`) etc.

## Alternatives and similar tools

- The approach from this crate can be reproduced with some amount of copy-pasting
  by manually feeding necessary inputs to a common parametric testing function.
  Optionally, these tests may be collected in a module for better structuring.
  The main downside of this approach is the amount of copy-pasting.
- Alternatively, multiple test cases may be run in a single `#[test]` (e.g., in a loop).
  This is fine for the large amount of small cases (e.g., mini-fuzzing), but may have downsides
  such as overflowing or overlapping logs and increased test runtimes.
- The [`test-case`] crate uses a similar approach to test case structuring, but differs
  in how test case inputs are specified. Subjectively, the approach used by this crate
  is more extensible and easier to read.
- [Property testing] / [`quickcheck`]-like frameworks provide a much more exhaustive approach
  to parameterized testing, but they require significantly more setup effort.
- [`rstest`] supports test casing and some test decorators (e.g., timeouts).
- [`nextest`] is an alternative test runner that supports most of the test decorators
  defined by this library. It does not use a code-based decorator config and
  does not allow for custom decorators. Tests produced with this library can be run by `cargo nextest`.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE)
or [MIT license](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in `test-casing` by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.

[custom test frameworks]: https://github.com/rust-lang/rust/issues/50297
[`test-case`]: https://crates.io/crates/test-case
[Property testing]: https://crates.io/crates/proptest
[`quickcheck`]: https://crates.io/crates/quickcheck
[`rstest`]: https://crates.io/crates/rstest
[`nextest`]: https://nexte.st/
