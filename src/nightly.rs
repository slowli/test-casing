//! Functionality gated by the `nightly` feature and requiring unstable features.

extern crate test;

use once_cell::sync::Lazy;

use std::{fmt, ops};
use test::{ShouldPanic, TestDesc, TestFn, TestName, TestType};

pub use test::assert_test_result;
pub type LazyTestCase = Lazy<TestDescAndFn>;

// Wrapper to overcome `!Sync` for `TestDescAndFn` caused by dynamic `TestFn` variants.
pub struct TestDescAndFn {
    inner: test::TestDescAndFn,
}

impl fmt::Debug for TestDescAndFn {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, formatter)
    }
}

// SAFETY: we only ever construct instances with a `Sync` variant of `TestFn`
// (namely `StaticTestFn`).
unsafe impl Sync for TestDescAndFn {}

impl TestDescAndFn {
    pub fn new(desc: TestDesc, testfn: fn() -> Result<(), String>) -> Self {
        Self {
            inner: test::TestDescAndFn {
                desc,
                testfn: TestFn::StaticTestFn(testfn),
            },
        }
    }
}

impl ops::Deref for TestDescAndFn {
    type Target = test::TestDescAndFn;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[doc(hidden)]
pub fn create_test_description<T: fmt::Debug>(
    is_unit_test: bool,
    base_name: &'static str,
    arg_names: impl crate::ArgNames<T>,
    cases: impl IntoIterator<Item = T>,
    index: usize,
) -> TestDesc {
    let path_in_crate = match base_name.split_once("::") {
        Some((_, path)) => path,
        None => "",
    };
    let test_args = crate::case(cases, index);
    let description = arg_names.print_with_args(&test_args);
    TestDesc {
        name: TestName::DynTestName(format!("{path_in_crate}::case_{index} [{description}]")),
        ignore: false,
        ignore_message: None,
        should_panic: ShouldPanic::No,
        compile_fail: false,
        no_run: false,
        test_type: if is_unit_test {
            TestType::UnitTest
        } else {
            TestType::IntegrationTest
        },
    }
}

pub fn set_ignore(desc: &mut TestDesc, message: Option<&'static str>) {
    desc.ignore = true;
    desc.ignore_message = message;
}

pub fn set_should_panic(desc: &mut TestDesc, message: Option<&'static str>) {
    desc.should_panic = match message {
        None => ShouldPanic::Yes,
        Some(message) => ShouldPanic::YesWithMessage(message),
    };
}

// We cannot declare a `const fn` to produce `LazyTestCase`s because the closure
// provided to `LazyTestCase::new()` cannot be inlined in a function. For the same reason,
// the closure in `TestDescAndFn::new()` is not inlined.
#[doc(hidden)]
#[macro_export]
macro_rules! declare_test_case {
    (
        base_name: $base_name:expr,
        arg_names: $arg_names:expr,
        cases: $cases:expr,
        index: $test_index:expr,
        $(ignore: $ignore:expr,)?
        $(panic_message: $panic_message:expr,)?
        testfn: $test_fn:path
    ) => {
        $crate::nightly::LazyTestCase::new(|| {
            let is_unit_test = ::core::option_env!("CARGO_TARGET_TMPDIR").is_none();
            let mut desc = $crate::nightly::create_test_description(
                is_unit_test,
                $base_name,
                $arg_names,
                $cases,
                $test_index,
            );
            $(
            $crate::nightly::set_ignore(&mut desc, $ignore);
            )?
            $(
            $crate::nightly::set_should_panic(&mut desc, $panic_message);
            )?
            $crate::nightly::TestDescAndFn::new(desc, || {
                $crate::nightly::assert_test_result($test_fn())
            })
        })
    };
}
