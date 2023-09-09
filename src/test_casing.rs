//! Support types for the `test_casing` macro.

use std::{fmt, iter::Fuse};

/// Obtains a test case from an iterator.
#[doc(hidden)] // used by the `#[test_casing]` macro; logically private
pub fn case<I: IntoIterator>(iter: I, index: usize) -> I::Item
where
    I::Item: fmt::Debug,
{
    iter.into_iter().nth(index).unwrap_or_else(|| {
        panic!("case #{index} not provided from the cases iterator");
    })
}

/// Allows printing named arguments together with their values to a `String`.
#[doc(hidden)] // used by the `#[test_casing]` macro; logically private
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
/// using the [`cases!`](crate::cases) macro.
///
/// # Examples
///
/// ```
/// # use test_casing::{cases, TestCases};
/// const NUMBER_CASES: TestCases<u32> = cases!([2, 3, 5, 8]);
/// const MORE_CASES: TestCases<u32> = cases! {
///     NUMBER_CASES.into_iter().chain([42, 555])
/// };
///
/// // The `cases!` macro can wrap a statement block:
/// const COMPLEX_CASES: TestCases<u32> = cases!({
///     use rand::{rngs::StdRng, Rng, SeedableRng};
///
///     let mut rng = StdRng::seed_from_u64(123);
///     (0..5).map(move |_| rng.gen())
/// });
/// ```
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
        *self
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
///
/// # Examples
///
/// See [`TestCases`](TestCases#examples) docs for the examples of usage.
#[macro_export]
macro_rules! cases {
    ($iter:expr) => {
        $crate::TestCases::<_>::new(|| {
            std::boxed::Box::new(core::iter::IntoIterator::into_iter($iter))
        })
    };
}

/// Cartesian product of several test cases.
///
/// For now, this supports products of 2..8 values. The provided [`IntoIterator`] expression
/// for each value must implement [`Clone`]. One way to do that is using [`TestCases`], which
/// wraps a lazy iterator initializer and is thus always [`Copy`]able.
///
/// # Examples
///
/// ```
/// # use test_casing::Product;
/// let product = Product((0..2, ["test", "other"]));
/// let values: Vec<_> = product.into_iter().collect();
/// assert_eq!(
///     values,
///     [(0, "test"), (0, "other"), (1, "test"), (1, "other")]
/// );
/// ```
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

#[cfg(doctest)]
doc_comment::doctest!("../README.md");

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

    #[test]
    fn unit_test_detection_works() {
        assert!(option_env!("CARGO_TARGET_TMPDIR").is_none());
    }
}
