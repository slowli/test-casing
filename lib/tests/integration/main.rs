//! Integration tests for crate functionality.

#![cfg_attr(feature = "nightly", feature(test, custom_test_frameworks))]
// Enable additional lints to ensure that the code produced by the macro doesn't raise warnings.
#![warn(missing_debug_implementations, missing_docs, bare_trait_objects)]
#![warn(clippy::all, clippy::pedantic)]

mod decorate;
mod test_casing;
