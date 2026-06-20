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

## Run Individual Benchmarks

```sh
# Thread signaling: Condvar vs Channel vs Park
cargo bench --bench thread_signaling

# Work distribution: Manual chunks vs Rayon (uniform + non-uniform workloads)
cargo bench --bench work_distribution

# Shared state: Arc vs Static LazyLock vs Box::leak vs thread::scope
cargo bench --bench shared_state

# Locking granularity: Coarse vs Fine vs Atomic
cargo bench --bench locking_granularity
```

## Benchmark 1: Thread Signaling

**File:** `benches/thread_signaling.rs`

Three ways to signal one thread from another:

| Approach | Mechanism | Heap Allocs | Tradeoff |
|---|---|---|---|
| **Condvar (Mutex)** | `Mutex<bool>` + `Condvar::wait/notify_one` | Mutex + Condvar init per run | Cheapest once set up. Can carry shared state. But the API is verbose and easy to screw up — forget the predicate loop and you're debugging spurious wakeups at 2am. |
| **MPSC Channel** | `mpsc::channel::<()>` + `send/recv` | 2 allocs (channel buf) | Simple API, channel can carry data. Heap alloc per-use. Fits multi-producer / single-consumer well. |
| **Thread Parking** | `thread::park/unpark` | 0 allocs | Zero allocation. Simplest code. No shared state — pure wakeup signal. Can lose wakeups if `unpark` fires before `park`. |

**Verdict:** Park if you just need a wakeup. Condvar if you need shared state too. Channel wins for multi-producer — but you pay a small alloc for the convenience.

## Benchmark 2: Work Distribution

**File:** `benches/work_distribution.rs`

Manual `thread::scope` with static chunking vs Rayon work-stealing, on two workload shapes.

### Workloads

| Workload | Behavior | Problem |
|---|---|---|
| **Uniform** | `*item += 10` per element | Predictable, same cost per item. No load imbalance. |
| **Non-Uniform** | If index % 7 == 0: 50x loop, else: add 1 | Branch bottleneck. Every 7th item ~50x heavier. Static chunks get unlucky. |

### Approaches

| Approach | Allocs | Tradeoff |
|---|---|---|
| **Manual `thread::scope` + `chunks_mut`** | 0 allocs (just thread spawn) | Zero overhead. Pre-divided chunks -- if workload spiky, one thread starves while others idle. Fixed partition per run. |
| **Rayon `par_iter_mut`** | Rayon threadpool allocs on first run | Work-stealing scheduler: idle threads steal from busy ones. Auto-tuned to core count. Heavier setup, but adapts to imbalance. |

### What you'll see

- **Uniform load:** Both approaches hover around the same speed. Manual chunks might sneak ahead — no work-stealing overhead, just raw thread spawning. Rayon's threadpool setup cost becomes noise at scale.
- **Spiky load:** Rayon pulls ahead. One thread gets unlucky with `% 7` hits and stalls; the others sit idle. Work-stealing redistributes the work and keeps everyone busy.

**Verdict:** Manual chunks if your work is predictable. Rayon if you've got spikes — it's what work-stealing was built for.

## Benchmark 3: Shared State

**File:** `benches/shared_state.rs`

Two sub-benchmarks — read-only (no Mutex) and read+write (with Mutex) — across 4 approaches:

| Approach | Lifetime | Allocs | Tradeoff |
|---|---|---|---|
| **Arc\<T\> / Arc\<Mutex\<T\>\>** | Last Arc drop | Arc inner (per run) | Clone cheap (atomic inc). Threads outlive spawn scope. Manual join handles. Simple ownership. |
| **Static LazyLock\<T\> / LazyLock\<Mutex\<T\>\>** | Program lifetime | Inner (once, lazy) | True singleton. Zero setup after init. Global state — test-unfriendly, no cleanup. |
| **Box::leak + OnceLock** | Program lifetime | Box (once, leaked) | No refcount, no lazy-init check after setup. Memory leak by design. |
| **thread::scope** | Scope block | 0 heap | Zero alloc. Borrow local — no `'static`. Auto-join. Most constrained (threads tied to scope). |

### Read-Only (no Mutex)

Threads just read a shared `&T`. No synchronization. This is purely measuring "how does the reference get to the thread."

| Scenario | What it measures |
|---|---|
| **Light (1 read/thread)** | Setup overhead: Arc clone, spawn, ref passing |
| **Heavy (1000 reads/thread)** | Per-access cost, amortized |

Heads up: LazyLock, leak, and scope should all be neck-and-neck — they all hand out `&T` for free. Arc pays an atomic increment per clone, which shows up in the numbers.

### Read+Write (with Mutex)

Same approaches, but now each thread locks and writes. Adds real contention.

| Scenario | What it measures |
|---|---|
| **Light (1 write/thread)** | Setup cost + lock acquire |
| **Heavy (1000 writes/thread)** | Mutex contention — this dominates everything |

Under heavy contention the approaches converge. The difference is really in the setup: scope skips the Arc clone, the handle Vec, and the join loop. Light workloads show this clearly.

**Verdict:** Use `scope` when threads don't outlive the caller. Use `Arc` when you need owned handles or flexible lifetimes. LazyLock and leak are for when a global singleton actually makes sense — which is rarer than you think.

## Benchmark 4: Locking Granularity

**File:** `benches/locking_granularity.rs`

Compare locking strategies for multi-field struct access across threads.

| Strategy | Structure | Lock ops | Tradeoff |
|---|---|---|---|
| **Coarse (1 Mutex)** | `Mutex<{a, b, c}>` | 1 lock/unlock per batch | Simple, serializes all field access even when unrelated. Best when all fields always accessed together. |
| **Fine (3 Mutexes)** | `{Mutex<a>, Mutex<b>, Mutex<c>}` | 1 lock/unlock per field touched | Parallel access to disjoint fields. Cost: 3x lock ops for all-fields update. Intermediate states visible. |
| **Atomic (AtomicI32)** | `{AtomicI32, AtomicI32, AtomicI32}` | 0 locks, CAS per field | Lock-free. Best throughput. No locking overhead. Can't atomically touch >1 field without extra work (CAS loop on struct copy). |

### Workloads

| Workload | Access Pattern | What It Shows |
|---|---|---|
| **Hot Field** | All threads hammer `a` (1000 inc each) | Same contention for all 3. Atomic fastest (CAS vs lock). Coarse ≈ Fine. |
| **Scattered** | Each thread targets different field (1000 inc) | Coarse serializes unrelated work. Fine only contends on `a`. Atomic = no contention. **Coarse worst case.** |
| **All Fields** | Each thread increments a, b, c in sequence (×1000) | Coarse: 1 lock/3 writes. Fine: 3 locks/1 write each. Atomic: 3 CAS ops. Coarse best when all fields needed together. |

### What to expect

- **Hot field:** Atomics blow past both mutex approaches. CAS is just a CPU instruction; locking involves the OS scheduler. Fine and coarse are essentially tied — same lock, same contention.
- **Scattered:** Atomics win again. Fine does well because threads mostly lock different fields. Coarse suffers — every thread queues on the same lock even though they want different data. This is the worst-case for coarse-grained locking.
- **All fields:** This one's interesting. Coarse locks once and does three writes. Fine locks and unlocks three times. Atomics do three separate CAS ops. Coarse can actually keep up with atomics here because it amortizes the lock. Fine pays for all that lock overhead.

**Verdict:** Coarse if you always need everything atomically and don't want to think about it. Fine if fields get accessed independently — but you pay for it when touching everything. Atomics when you can get away with single-field ops and want maximum throughput.

## Extra: Heap Profiling

All benchmarks use `stats_alloc` to print per-approach heap allocation stats to stdout during benchmark runs. Measures peak allocation (not cumulative), so you see the cost of each approach's setup.

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

## Comparison Matrix (TL;DR)

| Question | Winner | Why |
|---|---|---|
| Cheapest thread wakeup | `thread::park/unpark` | Zero alloc, zero state |
| Safest general sync | Condvar | Shared state + wake, but verbose |
| Easiest thread signaling | Channel | Simple API, carries data |
| Fastest uniform parallel work | Manual chunks | No scheduler overhead |
| Best spiky/unbalanced work | Rayon | Work-stealing balances load |
| Lowest parallel alloc footprint | Manual chunks | No threadpool allocs |
| Cheapest shared data setup (read-only) | `thread::scope` / `LazyLock` / `Box::leak` | Zero alloc, no clone, no sync |
| Cheapest shared data setup (read+write) | `thread::scope` | Zero alloc, no clone, auto-join; Arc pays clone + handle alloc |
| Flexible shared data ownership | `Arc<Mutex>` | Clone per thread, no lifetime constraints |
| Global singleton shared data | `LazyLock` / `Box::leak` | One-time init, static lifetime |
| Highest throughput (simple counter) | `AtomicI32` | CAS vs lock — no context switch |
| Best for unrelated field access | Fine-grained Mutexes | Parallel access to disjoint fields |
| Best for all-fields transaction | Coarse Mutex / Atomic | 1 lock vs 3 locks; atomic CAS for each field |

## License

MIT / Apache 2.0 (standard Rust dual license).
