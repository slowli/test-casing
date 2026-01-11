use std::fmt;

/// Allows printing named arguments together with their values to a `String`.
#[doc(hidden)] // used by the `__describe_test_case!` macro; logically private
pub trait ArgNames<T: fmt::Debug>: Copy + IntoIterator<Item = &'static str> {
    fn print_with_args(self, args: T) -> String;
}

macro_rules! impl_arg_names {
    ($n:tt => $($idx:tt: $arg_ty:ident),+) => {
        impl<$($arg_ty : fmt::Debug,)+> ArgNames<($($arg_ty,)+)> for [&'static str; $n] {
            fn print_with_args(self, args: ($($arg_ty,)+)) -> String {
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
    }
}

impl_arg_names!(1 => 0: T);
impl_arg_names!(2 => 0: T, 1: U);
impl_arg_names!(3 => 0: T, 1: U, 2: V);
impl_arg_names!(4 => 0: T, 1: U, 2: V, 3: W);
impl_arg_names!(5 => 0: T, 1: U, 2: V, 3: W, 4: X);
impl_arg_names!(6 => 0: T, 1: U, 2: V, 3: W, 4: X, 5: Y);
impl_arg_names!(7 => 0: T, 1: U, 2: V, 3: W, 4: X, 5: Y, 6: Z);
