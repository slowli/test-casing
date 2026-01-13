//! Tests tracing functionality.

use std::{
    cell::Cell,
    error,
    sync::atomic::{AtomicUsize, Ordering},
};

use test_casing::decorators::{self, Retry, RetryErrors, Trace};
use test_casing_macro::{decorate, test_casing};
use tracing_capture::{CaptureLayer, SharedStorage};
use tracing_subscriber::layer::SubscriberExt;

static TRACING: Trace = Trace::new("info,test_casing=debug");

#[derive(Debug)]
struct TracingWithStorage;

thread_local! {
    static TRACING_STORAGE: Cell<Option<SharedStorage>> = Cell::default();
}

impl TracingWithStorage {
    fn take_storage() -> SharedStorage {
        TRACING_STORAGE.take().expect("no injected storage")
    }
}

impl<R> decorators::DecorateTest<R> for TracingWithStorage {
    fn decorate_and_test<F: decorators::TestFn<R>>(&'static self, test_fn: F) -> R {
        let storage = SharedStorage::default();
        let subscriber = TRACING
            .create_subscriber()
            .with(CaptureLayer::new(&storage));
        let _guard = tracing::subscriber::set_default(subscriber);
        TRACING_STORAGE.set(Some(storage));
        test_fn()
    }
}

#[decorate(TracingWithStorage)]
#[test]
fn simple_test() {
    let storage = TracingWithStorage::take_storage();
    assert_has_decorator_event(&storage.lock());
}

fn assert_has_decorator_event(storage: &tracing_capture::Storage) {
    let event = storage
        .root_events()
        .find(|event| event.metadata().target() == "test_casing::decorators")
        .expect("no decorator event");

    assert_eq!(event.message(), Some("running decorated test"));
    let decorators = event.value("decorators").unwrap();
    assert!(
        decorators.is_debug(&(TracingWithStorage,)),
        "{decorators:?}"
    );
}

#[test_casing(2, [false, true])]
#[decorate(TracingWithStorage)]
fn parametric_test(flag: bool) {
    let storage = TracingWithStorage::take_storage();
    let storage = storage.lock();
    assert_has_decorator_event(&storage);

    let test_span = storage
        .root_spans()
        .find(|span| span.metadata().name() == "parametric_test")
        .expect("no test span");

    assert_eq!(test_span.metadata().target(), "traces");
    assert_eq!(
        test_span.value("case.index").unwrap().as_uint(),
        Some(u128::from(flag))
    );
    let span_flag = test_span.value("flag").unwrap();
    assert!(span_flag.is_debug(&flag), "{span_flag:?}");

    let start_event = test_span.events().next().expect("no start event");
    assert_eq!(start_event.metadata().target(), "traces");
    assert_eq!(start_event.message(), Some("started test"));
}

#[decorate(Retry::times(2), TracingWithStorage)]
#[test]
fn test_with_retries() {
    static RETRY_COUNTER: AtomicUsize = AtomicUsize::new(0);

    assert!(RETRY_COUNTER.fetch_add(1, Ordering::Relaxed) >= 2, "oops");
    let storage = TracingWithStorage::take_storage();
    assert_retry_events(&storage.lock(), true);
}

fn assert_retry_events(storage: &tracing_capture::Storage, should_panic: bool) {
    let attempt_spans = storage.all_spans().filter(|span| {
        span.metadata().target() == "test_casing::decorators"
            && span.metadata().name() == "test_attempt"
    });
    let attempt_spans: Vec<_> = attempt_spans.collect();
    assert_eq!(attempt_spans.len(), 3, "{attempt_spans:#?}");

    for (i, span) in attempt_spans.into_iter().enumerate() {
        let span_attempt = span.value("attempt").unwrap();
        assert_eq!(span_attempt.as_uint(), Some(i as u128));

        let (expected_msg, span_field) = if should_panic {
            ("test attempt panicked", "panic")
        } else {
            ("test attempt errored", "err")
        };
        let panic_event = span
            .events()
            .find(|event| event.message() == Some(expected_msg));
        if i < 2 {
            let panic_event = panic_event.expect("no error event");
            assert_eq!(panic_event.metadata().target(), "test_casing::decorators");
            assert_eq!(*panic_event.metadata().level(), tracing::Level::WARN);
            let panic_str = panic_event.value(span_field).unwrap();
            let panic_str = panic_str
                .as_str()
                .or_else(|| panic_str.as_debug_str())
                .unwrap();
            assert_eq!(panic_str, "oops");
        } else {
            assert!(panic_event.is_none());
        }
    }
}

const RETRY_ERRORS: RetryErrors<Box<dyn error::Error>> =
    Retry::times(2).on_error(|err| err.to_string().contains("oops"));

#[decorate(RETRY_ERRORS, TracingWithStorage)]
#[test]
fn test_with_retries_errors() -> Result<(), Box<dyn error::Error>> {
    static RETRY_COUNTER: AtomicUsize = AtomicUsize::new(0);

    if RETRY_COUNTER.fetch_add(1, Ordering::Relaxed) < 2 {
        return Err("oops".into());
    }

    let storage = TracingWithStorage::take_storage();
    assert_retry_events(&storage.lock(), false);
    Ok(())
}
