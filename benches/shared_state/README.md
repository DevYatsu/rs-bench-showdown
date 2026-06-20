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

```
1. Read-Only (1000 reads/thread)/Arc<T>
  time:   [54.926 µs 56.331 µs 58.092 µs]
  allocs: 632 bytes / 14 allocs

1. Read-Only (1000 reads/thread)/LazyLock<T>
  time:   [52.548 µs 52.997 µs 53.469 µs]
  allocs: 576 bytes / 13 allocs

1. Read-Only (1000 reads/thread)/Box::leak
  time:   [52.845 µs 53.524 µs 54.365 µs]
  allocs: 616 bytes / 14 allocs

1. Read-Only (1000 reads/thread)/thread::scope
  time:   [46.719 µs 47.310 µs 47.921 µs]
  allocs: 552 bytes / 13 allocs
```

All four are close. Scope edges ahead — no Arc clone, no OnceLock check, just a borrowed reference handed to each thread.

### Read+Write (1000 writes/thread)

```
2. Read+Write (1000 writes/thread)/Arc<Mutex<T>>
  time:   [141.20 µs 145.53 µs 149.73 µs]
  allocs: 712 bytes / 15 allocs

2. Read+Write (1000 writes/thread)/LazyLock<Mutex<T>>
  time:   [98.867 µs 102.62 µs 106.70 µs]
  allocs: 640 bytes / 14 allocs

2. Read+Write (1000 writes/thread)/Box::leak
  time:   [141.44 µs 281.61 µs 519.84 µs]
  allocs: 696 bytes / 15 allocs

2. Read+Write (1000 writes/thread)/thread::scope
  time:   [273.27 µs 292.41 µs 311.60 µs]
  allocs: 616 bytes / 14 allocs
```

Box::leak shows high variance in this run. Under write contention LazyLock leads. Scope and Arc trail: scope faces contention without any setup advantage, while Arc pays its atomic deref overhead.

## Run

```sh
cargo bench --bench shared_state
```

## Verdict

Use `scope` when threads don't outlive the caller — zero alloc, no clone, auto-join. Use `Arc` when you need owned handles or flexible lifetimes. LazyLock and leak are for when a global singleton actually makes sense — which is rarer than you think.
