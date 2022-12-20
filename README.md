# Parameterized Tests with Procedural Code Generation

`test-casing` is a minimalistic framework for generating tests for a given set of test cases.
In other words, it implements parameterized tests of reasonably low cardinality 
for the standard Rust test runner.

## Intended use cases

- Since a separate test wrapper is generated for each case, their number should be
  reasonably low (roughly speaking, no more than 20).
- Isolating each test case makes most sense if the cases involve some heavy lifting
  (spinning up a runtime, logging considerable amount of information, etc.).

## Usage

Add this to your `Crate.toml`:

```toml
[dev-dependencies]
test-casing = "0.1.0"
```

### Examples

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

// Async test cases are supported as well:
#[test_casing(4, [2, 3, 5, 8])]
#[async_std::test]
// ^ test attribute should be specified below the case spec
async fn test_async(number: i32) {
    assert!(number < 10);
}
```

See the crate docs for more examples of usage.

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
- [Property testing] / [`quickcheck`]-like frameworks provide much more exhaustive approach
  to parameterized testing, but they require significantly more setup effort.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE)
or [MIT license](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in `test-casing` by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.

[`test-case`]: https://crates.io/crates/test-case
[Property testing]: https://crates.io/crates/proptest
[`quickcheck`]: https://crates.io/crates/quickcheck
