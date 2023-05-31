//! Integration tests for the `decorate` macro.

use async_std::task;

use std::{
    error::Error,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
    thread,
    time::Duration,
};

use test_casing::{
    decorate, test_casing, DecorateTest, Retry, RetryErrors, Sequence, TestFn, Timeout,
};

#[test]
#[decorate(Timeout(Duration::from_secs(5)))]
fn with_inlined_timeout() {
    thread::sleep(Duration::from_millis(10));
}

const TIMEOUT: Timeout = Timeout(Duration::from_secs(3));

#[test]
#[decorate(TIMEOUT)]
fn with_timeout_constant() {
    thread::sleep(Duration::from_millis(10));
}

#[test]
#[decorate(TIMEOUT, Retry(2))]
fn with_mixed_decorators() {
    thread::sleep(Duration::from_millis(10));
}

#[test]
#[decorate(Retry(1))]
fn with_retries() {
    static COUNTER: AtomicU32 = AtomicU32::new(0);

    if COUNTER.fetch_add(1, Ordering::Relaxed) == 0 {
        panic!("Sometimes we all fail");
    }
}

#[test]
#[decorate(Retry(1))]
fn with_retries_and_error() -> Result<(), Box<dyn Error>> {
    static COUNTER: AtomicU32 = AtomicU32::new(0);

    if COUNTER.fetch_add(1, Ordering::Relaxed) == 0 {
        Err("Sometimes we all fail".into())
    } else {
        Ok(())
    }
}

const RETRY_ERRORS: RetryErrors<Box<dyn Error>> =
    Retry(1).on_error(|err| err.to_string().contains("retry"));

#[test]
#[decorate(RETRY_ERRORS)]
fn with_error_retries() -> Result<(), Box<dyn Error>> {
    static COUNTER: AtomicU32 = AtomicU32::new(0);

    if COUNTER.fetch_add(1, Ordering::Relaxed) == 0 {
        Err("please retry me".into())
    } else {
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct ShouldError(&'static str);

impl<E: ToString> DecorateTest<Result<(), E>> for ShouldError {
    fn decorate_and_test<F: TestFn<Result<(), E>>>(&'static self, test_fn: F) -> Result<(), E> {
        let Err(err) = test_fn() else {
            panic!("Expected test to error, but it completed successfully");
        };
        let err = err.to_string();
        if err.contains(self.0) {
            Ok(())
        } else {
            panic!(
                "Expected error message to contain `{}`, but it was: {err}",
                self.0
            );
        }
    }
}

#[test]
#[decorate(RETRY_ERRORS, ShouldError("oops"))] // also tests custom decorators
fn mismatched_error_with_error_retries() -> Result<(), Box<dyn Error>> {
    Err("oops".into())
}

#[test]
#[decorate(ShouldError("oops"), Retry(2))]
fn with_custom_decorator_and_retries() -> Result<(), &'static str> {
    static COUNTER: AtomicU32 = AtomicU32::new(0);

    match COUNTER.fetch_add(1, Ordering::Relaxed) {
        1 => Err("nothing to see here"),
        2 => Err("oops"),
        _ => Ok(()),
    }
}

#[test]
#[decorate(ShouldError("oops"))]
#[decorate(Retry(2))]
fn with_several_decorator_macros() -> Result<(), &'static str> {
    static COUNTER: AtomicU32 = AtomicU32::new(0);

    match COUNTER.fetch_add(1, Ordering::Relaxed) {
        1 => Err("nothing to see here"),
        2 => Err("oops"),
        _ => Ok(()),
    }
}

#[async_std::test]
#[decorate(Timeout(Duration::from_millis(100)), Retry(1))]
async fn async_test_with_timeout() {
    static COUNTER: AtomicU32 = AtomicU32::new(0);

    if COUNTER.fetch_add(1, Ordering::Relaxed) == 0 {
        task::sleep(Duration::from_millis(500)).await;
        // ^ will cause the test failure
    }
}

static SEQUENCE: Sequence = Sequence::new().abort_on_failure();

/// Checks that test in a `Sequence` are in fact sequential.
#[derive(Debug)]
struct SequenceChecker {
    is_running: AtomicBool,
}

impl SequenceChecker {
    const fn new() -> Self {
        Self {
            is_running: AtomicBool::new(false),
        }
    }

    fn start(&self) -> SequenceCheckerGuard<'_> {
        let prev_value = self.is_running.swap(true, Ordering::SeqCst);
        if prev_value {
            panic!("Sequential tests are not sequential!");
        }
        SequenceCheckerGuard {
            is_running: &self.is_running,
        }
    }
}

#[derive(Debug)]
struct SequenceCheckerGuard<'a> {
    is_running: &'a AtomicBool,
}

impl Drop for SequenceCheckerGuard<'_> {
    fn drop(&mut self) {
        self.is_running.store(false, Ordering::SeqCst);
    }
}

static SEQUENCE_CHECKER: SequenceChecker = SequenceChecker::new();

#[test]
#[should_panic(expected = "oops")]
#[decorate(&SEQUENCE)]
fn panicking_sequential_test() {
    let _guard = SEQUENCE_CHECKER.start();
    thread::sleep(Duration::from_millis(50));
    panic!("oops");
}

#[test]
#[decorate(&SEQUENCE)]
fn other_sequential_test() {
    let _guard = SEQUENCE_CHECKER.start();
    thread::sleep(Duration::from_millis(50));
}

#[async_std::test]
#[decorate(Retry(1), &SEQUENCE)]
async fn async_sequential_test() -> Result<(), Box<dyn Error>> {
    static COUNTER: AtomicU32 = AtomicU32::new(0);

    let _guard = SEQUENCE_CHECKER.start();
    task::sleep(Duration::from_millis(50)).await;
    if COUNTER.fetch_add(1, Ordering::Relaxed) == 0 {
        Err("oops".into())
    } else {
        Ok(())
    }
}

#[test_casing(3, ["1", "2", "3!"])]
#[decorate(Retry(1))]
fn cases_with_retries(s: &str) {
    // This is sloppy (the test case ordering is non-deterministic, so we can skip starting cases),
    // but sort of OK for the purpose.
    static IGNORE_ERROR: AtomicBool = AtomicBool::new(false);

    if IGNORE_ERROR.load(Ordering::SeqCst) {
        return;
    }

    let parse_result = s.parse::<usize>();
    if parse_result.is_err() {
        IGNORE_ERROR.store(true, Ordering::SeqCst);
    }
    parse_result.unwrap();
}
