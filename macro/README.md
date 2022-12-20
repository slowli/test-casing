# Proc Macro for `test-casing`

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%2FApache--2.0-blue)](https://github.com/slowli/test-casing#license)
![rust 1.65+ required](https://img.shields.io/badge/rust-1.65+-blue.svg?label=Required%20Rust)

**Documentation:**
[![crate docs (main)](https://img.shields.io/badge/main-yellow.svg?label=docs)](https://slowli.github.io/test-casing/test_casing_macro/)

`#[test_casing]` procedural macro for flattening a parameterized test suite into 
a collection of test cases. Used as a part of the [`test-casing`] library.

## Usage

Add this to your `Crate.toml`:

```toml
[dev-dependencies]
test-casing-macro = "0.1.0"
```

Note that the `test-casing` crate re-exports the proc macro. 
Thus, it is rarely necessary to use this crate as a direct dependency.

See `test-casing` docs for more details and examples of usage.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE)
or [MIT license](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in `test-casing` by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.

[`test-casing`]: https://crates.io/crates/test-casing
