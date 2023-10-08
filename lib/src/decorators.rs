//! Test decorator trait and implementations.
//!
//! # Overview
//!
//! A [test decorator](DecorateTest) takes a [tested function](TestFn) and calls it zero or more times,
//! perhaps with additional logic spliced between calls. Examples of decorators include [retries](Retry),
//! [`Timeout`]s and test [`Sequence`]s.
//!
//! Decorators are composable: `DecorateTest` is automatically implemented for a tuple with
//! 2..=8 elements where each element implements `DecorateTest`. The decorators in a tuple
//! are applied in the order of their appearance in the tuple.
//!
//! # Examples
//!
//! See [`decorate`](crate::decorate) macro docs for the examples of usage.

use std::{
    any::Any,
    fmt, panic,
    sync::{
        mpsc::{self, RecvTimeoutError},
        Mutex, PoisonError,
    },
    thread,
    time::Duration,
};

/// Tested function or closure.
///
/// This trait is automatically implemented for all functions without arguments.
pub trait TestFn<R>: Fn() -> R + panic::UnwindSafe + Send + Sync + Copy + 'static {}

impl<R, F> TestFn<R> for F where F: Fn() -> R + panic::UnwindSafe + Send + Sync + Copy + 'static {}

/// Test decorator.
///
/// See [module docs](index.html#overview) for the extended description.
///
/// # Examples
///
/// The following decorator implements a `#[should_panic]` analogue for errors.
///
/// ```
/// use test_casing::decorators::{DecorateTest, TestFn};
///
/// #[derive(Debug, Clone, Copy)]
/// pub struct ShouldError(pub &'static str);
///
/// impl<E: ToString> DecorateTest<Result<(), E>> for ShouldError {
///     fn decorate_and_test<F: TestFn<Result<(), E>>>(
///         &self,
///         test_fn: F,
///     ) -> Result<(), E> {
///         let Err(err) = test_fn() else {
///             panic!("Expected test to error, but it completed successfully");
///         };
///         let err = err.to_string();
///         if err.contains(self.0) {
///             Ok(())
///         } else {
///             panic!(
///                 "Expected error message to contain `{}`, but it was: {err}",
///                 self.0
///             );
///         }
///     }
/// }
///
/// // Usage:
/// # use test_casing::decorate;
/// # use std::error::Error;
/// #[test]
/// # fn eat_test_attribute() {}
/// #[decorate(ShouldError("oops"))]
/// fn test_with_an_error() -> Result<(), Box<dyn Error>> {
///     Err("oops, this test failed".into())
/// }
/// ```
pub trait DecorateTest<R>: panic::RefUnwindSafe + Send + Sync + 'static {
    /// Decorates the provided test function and runs the test.
    fn decorate_and_test<F: TestFn<R>>(&'static self, test_fn: F) -> R;
}

impl<R, T: DecorateTest<R>> DecorateTest<R> for &'static T {
    fn decorate_and_test<F: TestFn<R>>(&'static self, test_fn: F) -> R {
        (**self).decorate_and_test(test_fn)
    }
}

/// Object-safe version of [`DecorateTest`].
#[doc(hidden)] // used in the `decorate` proc macro; logically private
pub trait DecorateTestFn<R>: panic::RefUnwindSafe + Send + Sync + 'static {
    fn decorate_and_test_fn(&'static self, test_fn: fn() -> R) -> R;
}

impl<R: 'static, T: DecorateTest<R>> DecorateTestFn<R> for T {
    fn decorate_and_test_fn(&'static self, test_fn: fn() -> R) -> R {
        self.decorate_and_test(test_fn)
    }
}

/// [Test decorator](DecorateTest) that fails a wrapped test if it doesn't complete
/// in the specified [`Duration`].
///
/// # Examples
///
/// ```
/// use test_casing::{decorate, decorators::Timeout};
///
/// #[test]
/// # fn eat_test_attribute() {}
/// #[decorate(Timeout::secs(5))]
/// fn test_with_timeout() {
///     // test logic
/// }
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Timeout(pub Duration);

impl Timeout {
    /// Defines a timeout with the specified number of seconds.
    pub const fn secs(secs: u64) -> Self {
        Self(Duration::from_secs(secs))
    }

    /// Defines a timeout with the specified number of milliseconds.
    pub const fn millis(millis: u64) -> Self {
        Self(Duration::from_millis(millis))
    }
}

impl<R: Send + 'static> DecorateTest<R> for Timeout {
    #[allow(clippy::similar_names)]
    fn decorate_and_test<F: TestFn<R>>(&self, test_fn: F) -> R {
        let (output_sx, output_rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            output_sx.send(test_fn()).ok();
        });
        match output_rx.recv_timeout(self.0) {
            Ok(output) => {
                handle.join().unwrap();
                // ^ `unwrap()` is safe; the thread didn't panic before `send`ing the output,
                // and there's nowhere to panic after that.
                output
            }
            Err(RecvTimeoutError::Timeout) => {
                panic!("Timeout {:?} expired for the test", self.0);
            }
            Err(RecvTimeoutError::Disconnected) => {
                let panic_object = handle.join().unwrap_err();
                panic::resume_unwind(panic_object)
            }
        }
    }
}

/// [Test decorator](DecorateTest) that retries a wrapped test the specified number of times,
/// potentially with a delay between retries.
///
/// # Examples
///
/// ```
/// use test_casing::{decorate, decorators::Retry};
/// use std::time::Duration;
///
/// const RETRY_DELAY: Duration = Duration::from_millis(200);
///
/// #[test]
/// # fn eat_test_attribute() {}
/// #[decorate(Retry::times(3).with_delay(RETRY_DELAY))]
/// fn test_with_retries() {
///     // test logic
/// }
/// ```
#[derive(Debug)]
pub struct Retry {
    times: usize,
    delay: Duration,
}

impl Retry {
    /// Specified the number of retries. The delay between retries is zero.
    pub const fn times(times: usize) -> Self {
        Self {
            times,
            delay: Duration::ZERO,
        }
    }

    /// Specifies the delay between retries.
    #[must_use]
    pub const fn with_delay(self, delay: Duration) -> Self {
        Self { delay, ..self }
    }

    /// Converts this retry specification to only retry specific errors.
    pub const fn on_error<E>(self, matcher: fn(&E) -> bool) -> RetryErrors<E> {
        RetryErrors {
            inner: self,
            matcher,
        }
    }

    fn handle_panic(&self, attempt: usize, panic_object: Box<dyn Any + Send>) {
        if attempt < self.times {
            let panic_str = extract_panic_str(&panic_object).unwrap_or("");
            let punctuation = if panic_str.is_empty() { "" } else { ": " };
            println!("Test attempt #{attempt} panicked{punctuation}{panic_str}");
        } else {
            panic::resume_unwind(panic_object);
        }
    }

    fn run_with_retries<E: fmt::Display>(
        &self,
        test_fn: impl TestFn<Result<(), E>>,
        should_retry: fn(&E) -> bool,
    ) -> Result<(), E> {
        for attempt in 0..=self.times {
            println!("Test attempt #{attempt}");
            match panic::catch_unwind(test_fn) {
                Ok(Ok(())) => return Ok(()),
                Ok(Err(err)) => {
                    if attempt < self.times && should_retry(&err) {
                        println!("Test attempt #{attempt} errored: {err}");
                    } else {
                        return Err(err);
                    }
                }
                Err(panic_object) => {
                    self.handle_panic(attempt, panic_object);
                }
            }
            if self.delay > Duration::ZERO {
                thread::sleep(self.delay);
            }
        }
        Ok(())
    }
}

impl DecorateTest<()> for Retry {
    fn decorate_and_test<F: TestFn<()>>(&self, test_fn: F) {
        for attempt in 0..=self.times {
            println!("Test attempt #{attempt}");
            match panic::catch_unwind(test_fn) {
                Ok(()) => break,
                Err(panic_object) => {
                    self.handle_panic(attempt, panic_object);
                }
            }
            if self.delay > Duration::ZERO {
                thread::sleep(self.delay);
            }
        }
    }
}

impl<E: fmt::Display> DecorateTest<Result<(), E>> for Retry {
    fn decorate_and_test<F>(&self, test_fn: F) -> Result<(), E>
    where
        F: TestFn<Result<(), E>>,
    {
        self.run_with_retries(test_fn, |_| true)
    }
}

fn extract_panic_str(panic_object: &(dyn Any + Send)) -> Option<&str> {
    if let Some(panic_str) = panic_object.downcast_ref::<&'static str>() {
        Some(panic_str)
    } else if let Some(panic_string) = panic_object.downcast_ref::<String>() {
        Some(panic_string.as_str())
    } else {
        None
    }
}

/// [Test decorator](DecorateTest) that retries a wrapped test a certain number of times
/// only if an error matches the specified predicate.
///
/// Constructed using [`Retry::on_error()`].
///
/// # Examples
///
/// ```
/// use test_casing::{decorate, decorators::{Retry, RetryErrors}};
/// use std::error::Error;
///
/// const RETRY: RetryErrors<Box<dyn Error>> = Retry::times(3)
///     .on_error(|err| err.to_string().contains("retry please"));
///
/// #[test]
/// # fn eat_test_attribute() {}
/// #[decorate(RETRY)]
/// fn test_with_retries() -> Result<(), Box<dyn Error>> {
///     // test logic
/// #    Ok(())
/// }
/// ```
pub struct RetryErrors<E> {
    inner: Retry,
    matcher: fn(&E) -> bool,
}

impl<E> fmt::Debug for RetryErrors<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RetryErrors")
            .field("inner", &self.inner)
            .finish_non_exhaustive()
    }
}

impl<E: fmt::Display + 'static> DecorateTest<Result<(), E>> for RetryErrors<E> {
    fn decorate_and_test<F>(&self, test_fn: F) -> Result<(), E>
    where
        F: TestFn<Result<(), E>>,
    {
        self.inner.run_with_retries(test_fn, self.matcher)
    }
}

/// [Test decorator](DecorateTest) that makes runs of decorated tests sequential. The sequence
/// can optionally be aborted if a test in it fails.
///
/// The run ordering of tests in the sequence is not deterministic. This is because depending
/// on the command-line args that the test was launched with, not all tests in the sequence may run
/// at all.
///
/// # Examples
///
/// ```
/// use test_casing::{decorate, decorators::{Sequence, Timeout}};
///
/// static SEQUENCE: Sequence = Sequence::new().abort_on_failure();
///
/// #[test]
/// # fn eat_test_attribute() {}
/// #[decorate(&SEQUENCE)]
/// fn sequential_test() {
///     // test logic
/// }
///
/// #[test]
/// # fn eat_test_attribute2() {}
/// #[decorate(Timeout::secs(1), &SEQUENCE)]
/// fn other_sequential_test() {
///     // test logic
/// }
/// ```
#[derive(Debug, Default)]
pub struct Sequence {
    failed: Mutex<bool>,
    abort_on_failure: bool,
}

impl Sequence {
    /// Creates a new test sequence.
    pub const fn new() -> Self {
        Self {
            failed: Mutex::new(false),
            abort_on_failure: false,
        }
    }

    /// Makes the decorated tests abort immediately if one test from the sequence fails.
    #[must_use]
    pub const fn abort_on_failure(mut self) -> Self {
        self.abort_on_failure = true;
        self
    }

    fn decorate_inner<R, F: TestFn<R>>(
        &self,
        test_fn: F,
        ok_value: R,
        match_failure: fn(&R) -> bool,
    ) -> R {
        let mut guard = self.failed.lock().unwrap_or_else(PoisonError::into_inner);
        if *guard && self.abort_on_failure {
            println!("Skipping test because a previous test in the same sequence has failed");
            return ok_value;
        }

        let output = panic::catch_unwind(test_fn);
        *guard = output.as_ref().map_or(true, match_failure);
        drop(guard);
        output.unwrap_or_else(|panic_object| {
            panic::resume_unwind(panic_object);
        })
    }
}

impl DecorateTest<()> for Sequence {
    fn decorate_and_test<F: TestFn<()>>(&self, test_fn: F) {
        self.decorate_inner(test_fn, (), |_| false);
    }
}

impl<E: 'static> DecorateTest<Result<(), E>> for Sequence {
    fn decorate_and_test<F>(&self, test_fn: F) -> Result<(), E>
    where
        F: TestFn<Result<(), E>>,
    {
        self.decorate_inner(test_fn, Ok(()), Result::is_err)
    }
}

macro_rules! impl_decorate_test_for_tuple {
    ($($field:ident : $ty:ident),* => $last_field:ident : $last_ty:ident) => {
        impl<R, $($ty,)* $last_ty> DecorateTest<R> for ($($ty,)* $last_ty,)
        where
            $($ty: DecorateTest<R>,)*
            $last_ty: DecorateTest<R>,
        {
            fn decorate_and_test<Fn: TestFn<R>>(&'static self, test_fn: Fn) -> R {
                let ($($field,)* $last_field,) = self;
                $(
                let test_fn = move || $field.decorate_and_test(test_fn);
                )*
                $last_field.decorate_and_test(test_fn)
            }
        }
    };
}

impl_decorate_test_for_tuple!(=> a: A);
impl_decorate_test_for_tuple!(a: A => b: B);
impl_decorate_test_for_tuple!(a: A, b: B => c: C);
impl_decorate_test_for_tuple!(a: A, b: B, c: C => d: D);
impl_decorate_test_for_tuple!(a: A, b: B, c: C, d: D => e: E);
impl_decorate_test_for_tuple!(a: A, b: B, c: C, d: D, e: E => f: F);
impl_decorate_test_for_tuple!(a: A, b: B, c: C, d: D, e: E, f: F => g: G);
impl_decorate_test_for_tuple!(a: A, b: B, c: C, d: D, e: E, f: F, g: G => h: H);

#[cfg(test)]
mod tests {
    use std::{
        io,
        sync::{
            atomic::{AtomicU32, Ordering},
            Mutex,
        },
        time::Instant,
    };

    use super::*;

    #[test]
    #[should_panic(expected = "Timeout 100ms expired")]
    fn timeouts() {
        const TIMEOUT: Timeout = Timeout(Duration::from_millis(100));

        let test_fn: fn() = || thread::sleep(Duration::from_secs(1));
        TIMEOUT.decorate_and_test(test_fn);
    }

    #[test]
    fn retrying_with_delay() {
        const RETRY: Retry = Retry::times(1).with_delay(Duration::from_millis(100));

        fn test_fn() -> Result<(), &'static str> {
            static TEST_START: Mutex<Option<Instant>> = Mutex::new(None);

            let mut test_start = TEST_START.lock().unwrap();
            if let Some(test_start) = *test_start {
                assert!(test_start.elapsed() > RETRY.delay);
                Ok(())
            } else {
                *test_start = Some(Instant::now());
                Err("come again?")
            }
        }

        RETRY.decorate_and_test(test_fn).unwrap();
    }

    const RETRY: RetryErrors<io::Error> =
        Retry::times(2).on_error(|err| matches!(err.kind(), io::ErrorKind::AddrInUse));

    #[test]
    fn retrying_on_error() {
        static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

        fn test_fn() -> io::Result<()> {
            if TEST_COUNTER.fetch_add(1, Ordering::Relaxed) == 2 {
                Ok(())
            } else {
                Err(io::Error::new(
                    io::ErrorKind::AddrInUse,
                    "please retry later",
                ))
            }
        }

        let test_fn: fn() -> _ = test_fn;
        RETRY.decorate_and_test(test_fn).unwrap();
        assert_eq!(TEST_COUNTER.load(Ordering::Relaxed), 3);

        let err = RETRY.decorate_and_test(test_fn).unwrap_err();
        assert!(err.to_string().contains("please retry later"));
        assert_eq!(TEST_COUNTER.load(Ordering::Relaxed), 6);
    }

    #[test]
    fn retrying_on_error_failure() {
        static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

        fn test_fn() -> io::Result<()> {
            if TEST_COUNTER.fetch_add(1, Ordering::Relaxed) == 0 {
                Err(io::Error::new(io::ErrorKind::BrokenPipe, "oops"))
            } else {
                Ok(())
            }
        }

        let err = RETRY.decorate_and_test(test_fn).unwrap_err();
        assert!(err.to_string().contains("oops"));
        assert_eq!(TEST_COUNTER.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn sequential_tests() {
        static SEQUENCE: Sequence = Sequence::new();
        static ENTRY_COUNTER: AtomicU32 = AtomicU32::new(0);

        let first_test = || {
            let counter = ENTRY_COUNTER.fetch_add(1, Ordering::Relaxed);
            assert_eq!(counter, 0);
            thread::sleep(Duration::from_millis(10));
            ENTRY_COUNTER.store(0, Ordering::Relaxed);
            panic!("oops");
        };
        let second_test = || {
            let counter = ENTRY_COUNTER.fetch_add(1, Ordering::Relaxed);
            assert_eq!(counter, 0);
            thread::sleep(Duration::from_millis(20));
            ENTRY_COUNTER.store(0, Ordering::Relaxed);
            Ok::<_, io::Error>(())
        };

        let first_test_handle = thread::spawn(move || SEQUENCE.decorate_and_test(first_test));
        SEQUENCE.decorate_and_test(second_test).unwrap();
        first_test_handle.join().unwrap_err();
    }

    #[test]
    fn sequential_tests_with_abort() {
        static SEQUENCE: Sequence = Sequence::new().abort_on_failure();

        let failing_test =
            || Err::<(), _>(io::Error::new(io::ErrorKind::AddrInUse, "please try later"));
        let second_test = || unreachable!("Second test should not be called!");

        SEQUENCE.decorate_and_test(failing_test).unwrap_err();
        SEQUENCE.decorate_and_test(second_test);
    }

    // We need independent test counters for different tests, hence defining a function
    // via a macro.
    macro_rules! define_test_fn {
        () => {
            fn test_fn() -> Result<(), &'static str> {
                static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);
                match TEST_COUNTER.fetch_add(1, Ordering::Relaxed) {
                    0 => {
                        thread::sleep(Duration::from_secs(1));
                        Ok(())
                    }
                    1 => Err("oops"),
                    2 => Ok(()),
                    _ => unreachable!(),
                }
            }
        };
    }

    #[test]
    fn composing_decorators() {
        define_test_fn!();

        const DECORATORS: (Timeout, Retry) = (Timeout(Duration::from_millis(100)), Retry::times(2));

        DECORATORS.decorate_and_test(test_fn).unwrap();
    }

    #[test]
    fn making_decorator_into_trait_object() {
        define_test_fn!();

        static DECORATORS: &dyn DecorateTestFn<Result<(), &'static str>> =
            &(Timeout(Duration::from_millis(100)), Retry::times(2));

        DECORATORS.decorate_and_test_fn(test_fn).unwrap();
    }

    #[test]
    fn making_sequence_into_trait_object() {
        static SEQUENCE: Sequence = Sequence::new();
        static DECORATORS: &dyn DecorateTestFn<()> = &(&SEQUENCE,);

        DECORATORS.decorate_and_test_fn(|| {});
    }
}
