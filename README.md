# rs-bench-showdown

I got tired of guessing which approach was faster, so here's a benchmark suite comparing Rust concurrency patterns. Each test measures both runtime and heap allocations — because speed means nothing if it costs you a million tiny allocs.

## Prerequisites

```sh
# Report visualization (optional)
cargo install cargo-criterion
```

## Run All Benchmarks

```sh
cargo bench

# With HTML reports (criterion feature enabled in Cargo.toml)
# Reports land in target/criterion/report/
```

## Heap Profiling
All benchmarks use `stats_alloc` to print per-approach heap allocation stats to stdout during benchmark runs. Measures gross allocation within a Region (alloc - dealloc), so you see the cost of each approach's setup.

## About `black_box` and Compiler Optimizations

We use `std::hint::black_box` to stop the compiler from outsmarting the benchmarks. Without it, rustc can:

- **Inline and eliminate** reads or writes it can prove are redundant
- **Fold loops** when the result never gets used (dead code elimination is aggressive)
- **Specialize** generic code past the actual mechanism you're trying to measure

So these numbers show a **pessimistic-case comparison** — they force the compiler to actually do the operations.

In real production code, the compiler might optimize some approaches differently:

- An `Arc::clone` + deref that the optimizer can prove is immutable might disappear entirely
- A `Mutex` lock/unlock pair on a field nobody else touches might get lifted out of a loop
- Atomic operations with `Relaxed` ordering can fold or reorder in ways `Mutex` acquires can't (because `Mutex` acts as a compiler barrier)

**Bottom line:** These benchmarks rank approaches fairly in a controlled environment. In production, differences might shrink (compiler optimizes away overhead) or shift (some patterns optimize better than others). Profile your actual workload — these numbers are a starting point, not gospel.

## License

MIT / Apache 2.0 (standard Rust dual license).
