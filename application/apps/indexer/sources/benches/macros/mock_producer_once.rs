//! Provides macro to generate benchmarks with mock parser returning one value at a time using
//! various memory allocators

/// Macro to generate benchmarks with mock parser and source for various memory allocators.
/// The parser mock in this macro will return one item per call with `iter::once()` to replicate
/// the case with built in parsers.
///
/// # Usage:
/// The macro accepts the name of the benchmark function and the allocator, and generates a
/// benchmark function with given name using the given allocator
///
/// # Example:
/// ```
/// mod macros;
/// mocks_producer_once!(mocks_once_producer_stdalloc, std::alloc::System);
/// ```
#[macro_export]
macro_rules! mocks_producer_once {
    ($name:ident, $allocator:path) => {
        use ::criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
        use ::sources::producer::MessageProducer;
        use ::std::hint::black_box;
        use bench_utls::{bench_standrad_config, run_producer};
        use mocks::{mock_parser::MockParser, mock_source::MockByteSource};

        mod bench_utls;
        mod mocks;

        #[global_allocator]
        static GLOBAL: $allocator = $allocator;

        /// Runs Benchmarks replicating the producer loop within Chipmunk sessions, using mocks for
        /// [`parsers::Parser`] and [`sources::ByteSource`] to ensure that the measurements is for the
        /// producer loop only.
        ///
        /// The mock of [`parsers::Parser`] will return [`std::iter::once()`] replicating the behavior of
        /// the current built-in parsers in Chipmunk.
        ///
        /// NOTE: This benchmark suffers unfortunately from a lot of noise because we are running it with
        /// asynchronous runtime. This test is configured to reduce this amount of noise as possible,
        /// However it would be better to run it multiple time for double checking.
        fn $name(c: &mut Criterion) {
            let max_parse_calls = 50000;

            c.bench_with_input(
                BenchmarkId::new(std::stringify!($name), max_parse_calls),
                &(max_parse_calls),
                |bencher, &max| {
                    bencher
                        // It's important to spawn a new runtime on each run to ensure to reduce the
                        // potential noise produced from one runtime created at the start of all benchmarks
                        // only.
                        .to_async(tokio::runtime::Runtime::new().unwrap())
                        .iter_batched(
                            || {
                                // Exclude initiation time from benchmarks.
                                let parser = MockParser::new_once(max);
                                let byte_source = MockByteSource::new();
                                let producer =
                                    MessageProducer::new(parser, byte_source, black_box(None));

                                producer
                            },
                            |producer| run_producer(producer),
                            criterion::BatchSize::SmallInput,
                        )
                },
            );
        }

        criterion_group! {
            name = benches;
            config = bench_standrad_config();
            targets = $name
        }

        criterion_main!(benches);
    };
}
