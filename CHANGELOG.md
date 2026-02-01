# Changelog

All notable changes to this project will be documented in this file.
The project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

## 0.2.0-beta.1 - 2026-02-01

### Added

- Assert the test case count in a separate generated test. This protects against the scenario in which
  the number of test cases is increased, but the `test_casing` arg isn't updated.
- Add tracing decorator and output case info via `tracing` (rather than via printing it to stdout)
  if the corresponding crate feature is enabled.

### Changed

- Bump minimum supported Rust version to 1.82.

## 0.1.3 - 2024-03-03

### Fixed

- Fix `clippy::no_effect_underscore_binding` lint triggered by the generated code in Rust 1.76+.

## 0.1.2 - 2023-11-02

### Fixed

- Fix `unused_must_use` lint triggered for async functions without the explicit
  return value after the previous fix.
- Pin a version of the macro dependency in the main library so that it does not break
  in the future releases.

## 0.1.1 - 2023-10-08

### Fixed

- Fix `ignored_unit_patterns` Clippy lint triggered by the generated code in Rust 1.73+.

## 0.1.0 - 2023-06-03

The initial release of `test-casing`.
