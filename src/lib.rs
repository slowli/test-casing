//! Minimalistic framework for generating tests for a set of test cases.

#![cfg_attr(feature = "nightly", feature(custom_test_frameworks, test))]
// Linter settings.
#![warn(missing_debug_implementations, missing_docs, bare_trait_objects)]
#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::must_use_candidate, clippy::module_name_repetitions)]

use std::{fmt, iter::Fuse};

pub use test_casing_macro::test_casing;

/// FIXME
#[cfg(feature = "nightly")]
pub mod nightly {
    extern crate test;

    use once_cell::sync::Lazy;

    use std::{fmt, ops};

    #[doc(hidden)]
    pub use test::assert_test_result;
    #[doc(hidden)]
    pub type LazyTestCase = Lazy<TestDescAndFn>;

    use test::{ShouldPanic, TestDesc, TestFn, TestName, TestType};

    // Wrapper to overcome `!Sync` for `TestDescAndFn` caused by dynamic `TestFn` variants.
    #[doc(hidden)]
    pub struct TestDescAndFn {
        inner: test::TestDescAndFn,
    }

    impl fmt::Debug for TestDescAndFn {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            fmt::Debug::fmt(&self.inner, formatter)
        }
    }

    // SAFETY: we only ever construct instances with a `Sync` variant of `TestFn`
    // (namely `StaticTestFn`)
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

    // FIXME
    const fn detect_test_type(_path: &str) -> TestType {
        TestType::IntegrationTest
    }

    #[doc(hidden)]
    pub fn create_test_description<T: fmt::Debug>(
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
            test_type: detect_test_type("test"),
        }
    }

    #[doc(hidden)]
    pub fn set_ignore(desc: &mut TestDesc, message: Option<&'static str>) {
        desc.ignore = true;
        desc.ignore_message = message;
    }

    #[doc(hidden)]
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
                let mut desc = $crate::nightly::create_test_description(
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
}

#[doc(hidden)]
pub fn case<I: IntoIterator>(iter: I, index: usize) -> I::Item
where
    I::Item: fmt::Debug,
{
    iter.into_iter().nth(index).unwrap_or_else(|| {
        panic!("case #{index} not provided from the cases iterator");
    })
}

#[doc(hidden)]
pub trait ArgNames<T: fmt::Debug>: Copy + IntoIterator<Item = &'static str> {
    fn print_with_args(self, args: &T) -> String;
}

impl<T: fmt::Debug> ArgNames<T> for [&'static str; 1] {
    fn print_with_args(self, args: &T) -> String {
        format!("{name} = {args:?}", name = self[0])
    }
}

macro_rules! impl_arg_names {
    ($n:tt => $($idx:tt: $arg_ty:ident),+) => {
        impl<$($arg_ty : fmt::Debug,)+> ArgNames<($($arg_ty,)+)> for [&'static str; $n] {
            fn print_with_args(self, args: &($($arg_ty,)+)) -> String {
                use std::fmt::Write as _;

                let mut buffer = String::new();
                $(
                write!(buffer, "{} = {:?}", self[$idx], args.$idx).unwrap();
                if $idx + 1 < self.len() {
                    buffer.push_str(", ");
                }
                )+
                buffer
            }
        }
    };
}

impl_arg_names!(2 => 0: T, 1: U);
impl_arg_names!(3 => 0: T, 1: U, 2: V);
impl_arg_names!(4 => 0: T, 1: U, 2: V, 3: W);
impl_arg_names!(5 => 0: T, 1: U, 2: V, 3: W, 4: X);
impl_arg_names!(6 => 0: T, 1: U, 2: V, 3: W, 4: X, 5: Y);
impl_arg_names!(7 => 0: T, 1: U, 2: V, 3: W, 4: X, 5: Y, 6: Z);

/// Container for test cases based on a lazily evaluated iterator. Should be constructed
/// using the [`cases!`] macro.
pub struct TestCases<T> {
    lazy: fn() -> Box<dyn Iterator<Item = T>>,
}

impl<T> fmt::Debug for TestCases<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("TestCases").finish_non_exhaustive()
    }
}

impl<T> Clone for TestCases<T> {
    fn clone(&self) -> Self {
        Self { lazy: self.lazy }
    }
}

impl<T> Copy for TestCases<T> {}

impl<T> TestCases<T> {
    /// Creates a new set of test cases.
    pub const fn new(lazy: fn() -> Box<dyn Iterator<Item = T>>) -> Self {
        Self { lazy }
    }
}

impl<T> IntoIterator for TestCases<T> {
    type Item = T;
    type IntoIter = Box<dyn Iterator<Item = T>>;

    fn into_iter(self) -> Self::IntoIter {
        (self.lazy)()
    }
}

/// Creates [`TestCases`] based on the provided expression implementing [`IntoIterator`]
/// (e.g., an array, a range or an iterator).
#[macro_export]
macro_rules! cases {
    ($iter:expr) => {
        $crate::TestCases::<_>::new(|| {
            std::boxed::Box::new(core::iter::IntoIterator::into_iter($iter))
        })
    };
}

/// Cartesian product of several test cases.
#[derive(Debug, Clone, Copy)]
pub struct Product<Ts>(pub Ts);

impl<T, U> IntoIterator for Product<(T, U)>
where
    T: Clone + IntoIterator,
    U: Clone + IntoIterator,
{
    type Item = (T::Item, U::Item);
    type IntoIter = ProductIter<T, U>;

    fn into_iter(self) -> Self::IntoIter {
        let (_, second) = &self.0;
        let second = second.clone();
        ProductIter {
            sources: self.0,
            first_idx: 0,
            second_iter: second.into_iter().fuse(),
            is_finished: false,
        }
    }
}

macro_rules! impl_product {
    ($head:ident: $head_ty:ident, $($tail:ident: $tail_ty:ident),+) => {
        impl<$head_ty, $($tail_ty,)+> IntoIterator for Product<($head_ty, $($tail_ty,)+)>
        where
            $head_ty: 'static + Clone + IntoIterator,
            $($tail_ty: 'static + Clone + IntoIterator,)+
        {
            type Item = ($head_ty::Item, $($tail_ty::Item,)+);
            type IntoIter = Box<dyn Iterator<Item = Self::Item>>;

            fn into_iter(self) -> Self::IntoIter {
                let ($head, $($tail,)+) = self.0;
                let tail = Product(($($tail,)+));
                let iter = Product(($head, tail))
                    .into_iter()
                    .map(|($head, ($($tail,)+))| ($head, $($tail,)+));
                Box::new(iter)
            }
        }
    };
}

impl_product!(t: T, u: U, v: V);
impl_product!(t: T, u: U, v: V, w: W);
impl_product!(t: T, u: U, v: V, w: W, x: X);
impl_product!(t: T, u: U, v: V, w: W, x: X, y: Y);
impl_product!(t: T, u: U, v: V, w: W, x: X, y: Y, z: Z);

/// Iterator over test cases in [`Product`].
#[derive(Debug)]
pub struct ProductIter<T: IntoIterator, U: IntoIterator> {
    sources: (T, U),
    first_idx: usize,
    second_iter: Fuse<U::IntoIter>,
    is_finished: bool,
}

impl<T, U> Iterator for ProductIter<T, U>
where
    T: Clone + IntoIterator,
    U: Clone + IntoIterator,
{
    type Item = (T::Item, U::Item);

    fn next(&mut self) -> Option<Self::Item> {
        if self.is_finished {
            return None;
        }

        loop {
            if let Some(second_case) = self.second_iter.next() {
                let mut first_iter = self.sources.0.clone().into_iter();
                let Some(first_case) = first_iter.nth(self.first_idx) else {
                    self.is_finished = true;
                    return None;
                };
                return Some((first_case, second_case));
            }
            self.first_idx += 1;
            self.second_iter = self.sources.1.clone().into_iter().fuse();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn cartesian_product() {
        let numbers = cases!(0..3);
        let strings = cases!(["0", "1"]);
        let cases: Vec<_> = Product((numbers, strings)).into_iter().collect();
        assert_eq!(
            cases.as_slice(),
            [(0, "0"), (0, "1"), (1, "0"), (1, "1"), (2, "0"), (2, "1")]
        );

        let booleans = [false, true];
        let cases: HashSet<_> = Product((numbers, strings, booleans)).into_iter().collect();
        assert_eq!(cases.len(), 12); // 3 * 2 * 2
    }
}
