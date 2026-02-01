# Parameterized Rust Tests & Test Decorators

[![Build status](https://github.com/slowli/test-casing/actions/workflows/ci.yml/badge.svg)](https://github.com/slowli/test-casing/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%2FApache--2.0-blue)](https://github.com/slowli/test-casing#license)

`test-casing` is a minimalistic Rust framework for generating tests for a given set of test cases
and decorating them to add retries, timeouts, sequential test processing etc.
In other words, the framework implements:

- Parameterized tests of reasonably low cardinality for the standard Rust test runner
- Fully code-based, composable and extensible test decorators.

## Contents

- [`lib`](lib): The main crate
- [`macro`](macro): Proc macros supporting the main crate.

## Contributing

All contributions are welcome! See [the contributing guide](CONTRIBUTING.md) to help
you get involved.

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
