//! Test decorator trait and implementations.

#![allow(missing_docs)] // FIXME

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

pub trait TestFn<R>: Fn() -> R + panic::UnwindSafe + Send + Sync + Copy + 'static {}

impl<R, F> TestFn<R> for F where F: Fn() -> R + panic::UnwindSafe + Send + Sync + Copy + 'static {}

pub trait DecorateTest<R>: panic::RefUnwindSafe + Send + Sync + 'static {
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

#[derive(Debug, Clone, Copy)]
pub struct Timeout(pub Duration);

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

#[derive(Debug)]
pub struct Retry(pub usize);

impl Retry {
    pub const fn on_error<E>(self, matcher: fn(&E) -> bool) -> RetryErrors<E> {
        RetryErrors {
            inner: self,
            matcher,
        }
    }

    fn handle_panic(&self, attempt: usize, panic_object: Box<dyn Any + Send>) {
        if attempt < self.0 {
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
        for attempt in 0..=self.0 {
            println!("Test attempt #{attempt}");
            match panic::catch_unwind(test_fn) {
                Ok(Ok(())) => return Ok(()),
                Ok(Err(err)) => {
                    if attempt < self.0 && should_retry(&err) {
                        println!("Test attempt #{attempt} errored: {err}");
                    } else {
                        return Err(err);
                    }
                }
                Err(panic_object) => {
                    self.handle_panic(attempt, panic_object);
                }
            }
        }
        Ok(())
    }
}

impl DecorateTest<()> for Retry {
    fn decorate_and_test<F: TestFn<()>>(&self, test_fn: F) {
        for attempt in 0..=self.0 {
            println!("Test attempt #{attempt}");
            match panic::catch_unwind(test_fn) {
                Ok(()) => break,
                Err(panic_object) => {
                    self.handle_panic(attempt, panic_object);
                }
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

#[derive(Debug, Default)]
pub struct Sequence {
    failed: Mutex<bool>,
    abort_on_failure: bool,
}

impl Sequence {
    pub const fn new() -> Self {
        Self {
            failed: Mutex::new(false),
            abort_on_failure: false,
        }
    }

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

// TODO: use direct ordering of decorators?
macro_rules! impl_decorate_test_for_tuple {
    ($($field:ident : $ty:ident),+ => $($layer:ident),* => $final:ident) => {
        impl<R, $($ty,)+> DecorateTest<R> for ($($ty,)+)
        where
            $($ty: DecorateTest<R>,)+
        {
            fn decorate_and_test<F: TestFn<R>>(&'static self, test_fn: F) -> R {
                let ($($field,)+) = self;
                $(
                let test_fn = move || $layer.decorate_and_test(test_fn);
                )*
                $final.decorate_and_test(test_fn)
            }
        }
    };
}

impl_decorate_test_for_tuple!(a: A => => a);
impl_decorate_test_for_tuple!(a: A, b: B => b => a);
impl_decorate_test_for_tuple!(a: A, b: B, c: C => c, b => a);
impl_decorate_test_for_tuple!(a: A, b: B, c: C, d: D => d, c, b => a);

#[cfg(test)]
mod tests {
    use std::{
        io,
        sync::atomic::{AtomicU32, Ordering},
    };

    use super::*;

    #[test]
    #[should_panic(expected = "Timeout 100ms expired")]
    fn timeouts() {
        const TIMEOUT: Timeout = Timeout(Duration::from_millis(100));

        let test_fn: fn() = || thread::sleep(Duration::from_secs(1));
        TIMEOUT.decorate_and_test(test_fn);
    }

    const RETRY: RetryErrors<io::Error> =
        Retry(2).on_error(|err| matches!(err.kind(), io::ErrorKind::AddrInUse));

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

        const DECORATORS: (Retry, Timeout) = (Retry(2), Timeout(Duration::from_millis(100)));

        DECORATORS.decorate_and_test(test_fn).unwrap();
    }

    #[test]
    fn making_decorator_into_trait_object() {
        define_test_fn!();

        static DECORATORS: &dyn DecorateTestFn<Result<(), &'static str>> =
            &(Retry(2), Timeout(Duration::from_millis(100)));

        DECORATORS.decorate_and_test_fn(test_fn).unwrap();
    }

    #[test]
    fn making_sequence_into_trait_object() {
        static SEQUENCE: Sequence = Sequence::new();
        static DECORATORS: &dyn DecorateTestFn<()> = &(&SEQUENCE,);

        DECORATORS.decorate_and_test_fn(|| {});
    }
}
