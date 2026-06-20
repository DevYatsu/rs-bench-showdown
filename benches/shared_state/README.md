# Shared State

Two sub-benchmarks — read-only (no Mutex) and read+write (with Mutex) — across 4 approaches. Each uses 4 threads doing 1000 ops each.

## Approaches

| Approach | Lifetime | Allocs (read-only) | Allocs (read+write) | Tradeoff |
|---|---|---|---|---|
| **Arc\<T\> / Arc\<Mutex\<T\>\>** | Last Arc drop | 632 bytes / 14 allocs | 712 bytes / 15 allocs | Clone cheap (atomic inc). Threads outlive spawn scope. Manual join handles. |
| **Static LazyLock\<T\> / LazyLock\<Mutex\<T\>\>** | Program lifetime | 576 bytes / 13 allocs | 640 bytes / 14 allocs | True singleton. Zero setup after init. Global state — test-unfriendly. |
| **Box::leak** | Program lifetime | 616 bytes / 14 allocs | 696 bytes / 15 allocs | No refcount, no lazy-init check. Memory leak by design. |
| **thread::scope** | Scope block | 552 bytes / 13 allocs | 616 bytes / 14 allocs | Zero heap alloc. Borrow local — no `'static`. Auto-join. Most constrained. |

## Results (Apple M3, 8 cores, 16 GB)

### Read-Only (1000 reads/thread)

| Approach | Time | Allocs |
|---|---|---|
| Arc\<T\> | 54.926 — 58.092 µs | 632 B / 14 allocs |
| LazyLock\<T\> | 52.548 — 53.469 µs | 576 B / 13 allocs |
| Box::leak | 52.845 — 54.365 µs | 616 B / 14 allocs |
| thread::scope | 46.719 — 47.921 µs | 552 B / 13 allocs |

All four are close. Scope edges ahead — no Arc clone, no OnceLock check, just a borrowed reference handed to each thread.

### Read+Write (1000 writes/thread)

| Approach | Time | Allocs |
|---|---|---|
| Arc\<Mutex\<T\>\> | 141.20 — 149.73 µs | 712 B / 15 allocs |
| LazyLock\<Mutex\<T\>\> | 98.867 — 106.70 µs | 640 B / 14 allocs |
| Box::leak | 141.44 — 519.84 µs* | 696 B / 15 allocs |
| thread::scope | 273.27 — 311.60 µs | 616 B / 14 allocs |

\*Box::leak shows high variance in this run — results should be treated with caution.

Under write contention the ordering shifts. LazyLock leads here. Scope and Arc trail: scope faces contention without any setup advantage to offset it, while Arc pays its atomic deref overhead.

## Run

```sh
cargo bench --bench shared_state
```

## Verdict

Use `scope` when threads don't outlive the caller — zero alloc, no clone, auto-join. Use `Arc` when you need owned handles or flexible lifetimes. LazyLock and leak are for when a global singleton actually makes sense — which is rarer than you think.
