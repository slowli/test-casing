//! Tracing decorators.

use std::{env, fmt};

use tracing::{level_filters::LevelFilter, Dispatch, Event, Subscriber};
use tracing_subscriber::{
    field::RecordFields,
    fmt::{format, format::Writer, FmtContext, FormatEvent, FormatFields, TestWriter},
    registry::LookupSpan,
    EnvFilter, FmtSubscriber,
};

use crate::decorators::{DecorateTest, TestFn};

#[derive(Debug)]
enum Either<L, R> {
    Left(L),
    Right(R),
}

impl<'w, L, R> FormatFields<'w> for Either<L, R>
where
    L: FormatFields<'w>,
    R: FormatFields<'w>,
{
    fn format_fields<F: RecordFields>(&self, writer: Writer<'w>, fields: F) -> fmt::Result {
        match self {
            Self::Left(formatter) => formatter.format_fields(writer, fields),
            Self::Right(formatter) => formatter.format_fields(writer, fields),
        }
    }
}

impl<S, N, L, R> FormatEvent<S, N> for Either<L, R>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
    L: FormatEvent<S, N>,
    R: FormatEvent<S, N>,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        match self {
            Self::Left(formatter) => formatter.format_event(ctx, writer, event),
            Self::Right(formatter) => formatter.format_event(ctx, writer, event),
        }
    }
}

type TestSubscriber = FmtSubscriber<
    Either<format::Pretty, format::DefaultFields>,
    Either<format::Format<format::Pretty>, format::Format>,
    EnvFilter,
    TestWriter,
>;

/// Decorator that enables tracing for tests.
///
/// # Examples
///
/// ```no_run
/// use test_casing::{decorate, decorators::Trace};
///
/// // Tracing output configuration. Allows to specify the default log directives
/// // (can be overridden by the `RUST_LOG` env var in the runtime), and to configure
/// // more concise or pretty output.
/// static TRACE: Trace = Trace::new("info,test_crate=trace").pretty();
///
/// #[decorate(TRACE)]
/// #[test]
/// fn some_test() {
///     // Test logic...
///     tracing::info!("logging event");
/// }
/// ```
#[cfg_attr(docsrs, doc(cfg(feature = "tracing")))]
#[derive(Debug, Clone, Copy)]
pub struct Trace {
    directives: Option<&'static str>,
    pretty: bool,
    global: bool,
}

impl Trace {
    /// Creates a decorator with the specified directives. The directives can be overridden by the `RUST_LOG`
    /// env variable in runtime.
    pub const fn new(directives: &'static str) -> Self {
        Self {
            directives: Some(directives),
            pretty: false,
            global: false,
        }
    }

    /// Enables pretty formatting for the tracing events.
    #[must_use]
    pub const fn pretty(mut self) -> Self {
        self.pretty = true;
        self
    }

    /// Sets up the tracing subscriber globally (vs the default thread-local setup).
    /// This is only beneficial for multithreaded tests, and may have undesired side effects.
    #[must_use]
    pub const fn global(mut self) -> Self {
        self.global = true;
        self
    }

    /// Creates a subscriber based on the configured params. This is useful in order to reuse [`Trace`]
    /// logic in more complex decorators (e.g., ones that capture tracing spans / events).
    pub fn create_subscriber(self) -> impl Subscriber + for<'a> LookupSpan<'a> {
        self.create_subscriber_inner()
    }

    fn create_subscriber_inner(self) -> TestSubscriber {
        let env = env::var("RUST_LOG").ok();
        let env = env.as_deref().or(self.directives).unwrap_or_default();
        let env_filter = EnvFilter::builder()
            .with_default_directive(LevelFilter::INFO.into())
            .parse_lossy(env);
        FmtSubscriber::builder()
            .with_test_writer()
            .with_env_filter(env_filter)
            .fmt_fields(if self.pretty {
                Either::Left(format::Pretty::default())
            } else {
                Either::Right(format::DefaultFields::default())
            })
            .map_event_format(|fmt| {
                if self.pretty {
                    Either::Left(fmt.pretty())
                } else {
                    Either::Right(fmt)
                }
            })
            .finish()
    }
}

impl<R> DecorateTest<R> for Trace {
    fn decorate_and_test<F: TestFn<R>>(&'static self, test_fn: F) -> R {
        let subscriber = self.create_subscriber_inner();
        let _guard = if self.global {
            if tracing::subscriber::set_global_default(subscriber).is_err() {
                let is_test_subscriber =
                    tracing::dispatcher::get_default(Dispatch::is::<TestSubscriber>);
                if !is_test_subscriber {
                    tracing::warn!("could not set up global tracing subscriber");
                }
            }
            None
        } else {
            Some(tracing::subscriber::set_default(subscriber))
        };
        test_fn()
    }
}
