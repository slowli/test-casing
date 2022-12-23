//! UI tests for various compilation failures.

#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
}

#[cfg(feature = "nightly")]
#[test]
fn nightly_ui() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui-nightly/*.rs");
}
