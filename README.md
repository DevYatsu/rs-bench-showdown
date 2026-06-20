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

## Individual Benchmarks

Each benchmark lives in its own subdirectory under `benches/` with a dedicated README:

| Benchmark | Directory | What it compares |
|---|---|---|
| **Thread Signaling** | `benches/thread_signaling/` | Condvar vs MPSC Channel vs thread::park/unpark |
| **Work Distribution** | `benches/work_distribution/` | Manual `thread::scope` chunks vs Rayon work-stealing |
| **Shared State** | `benches/shared_state/` | Arc vs LazyLock vs Box::leak vs thread::scope (read-only + read+write) |
| **Locking Granularity** | `benches/locking_granularity/` | Coarse Mutex vs Fine Mutexes vs Atomic per field |

```sh
# Run a single benchmark
cargo bench --bench thread_signaling
cargo bench --bench work_distribution
cargo bench --bench shared_state
cargo bench --bench locking_granularity
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

## Comparison Matrix

| Question | Winner | Why |
|---|---|---|
| Cheapest thread wakeup | `thread::park/unpark` | Zero alloc, zero state |
| Safest general sync | Condvar | Shared state + wake, but verbose |
| Easiest thread signaling | Channel | Simple API, carries data |
| Fastest uniform parallel work | Manual chunks | No scheduler overhead |
| Best spiky/unbalanced work | Rayon | Work-stealing balances load |
| Lowest parallel alloc footprint | Manual chunks | No threadpool allocs |
| Cheapest shared data setup (read-only) | `thread::scope` | Zero alloc, no clone, no sync |
| Cheapest shared data setup (read+write) | *varies* | Results fluctuate between runs — profile your workload |
| Flexible shared data ownership | `Arc<Mutex>` | Clone per thread, no lifetime constraints |
| Global singleton shared data | `LazyLock` / `Box::leak` | One-time init, static lifetime |
| Highest throughput (simple counter) | `AtomicI32` | CAS vs lock — no context switch |
| Best for unrelated field access | Fine-grained Mutexes | Parallel access to disjoint fields |
| Best for all-fields transaction | Atomic / Coarse Mutex | 1 lock vs 3 locks; atomic CAS per field |

## Results

Results in each sub-benchmark's README were measured on **Apple M3, 8 cores, 16 GB**. Your mileage will vary by hardware and workload shape.

## License

MIT / Apache 2.0 (standard Rust dual license).
