# Locking Granularity

Compare locking strategies for multi-field struct access across threads. Each benchmark runs 1000 ops per thread with 4 threads.

## Strategies

| Strategy | Structure | Lock ops | Allocs | Tradeoff |
|---|---|---|---|---|
| **Coarse (1 Mutex)** | `Mutex<{a, b, c}>` | 1 lock/unlock per batch | 712 B / 15 allocs | Simple, serializes all field access. Best when all fields accessed together. |
| **Fine (3 Mutexes)** | `{Mutex<a>, Mutex<b>, Mutex<c>}` | 1 lock/unlock per field | 736 B / 15 allocs | Parallel access to disjoint fields. 3x lock ops for all-fields update. |
| **Atomic (AtomicI32)** | `{AtomicI32, AtomicI32, AtomicI32}` | 0 locks, CAS per field | 640 B / 14 allocs | Lock-free. No locking overhead. Can't atomically touch >1 field. |

## Workloads

| Workload | Pattern | What It Shows |
|---|---|---|
| **Hot Field** | All threads hammer field `a` (1000 inc each) | Same contention for all 3. Atomic fastest. |
| **Scattered** | Each thread targets a different field (1000 inc) | Coarse serializes unrelated work. **Coarse worst case.** |
| **All Fields** | Each thread increments a, b, c (×1000) | Coarse: 1 lock/3 writes. Fine: 3 locks. Atomic: 3 CAS. |

## Results (Apple M3, 8 cores, 16 GB)

### Hot Field

```
1. Hot Field/Coarse (1 Mutex)
  time:   [77.582 µs 77.923 µs 78.241 µs]
  allocs: 712 bytes / 15 allocs

1. Hot Field/Fine (3 Mutexes)
  time:   [80.867 µs 87.608 µs 97.139 µs]
  allocs: 736 bytes / 15 allocs

1. Hot Field/Atomic
  time:   [43.371 µs 45.375 µs 47.447 µs]
  allocs: 640 bytes / 14 allocs
```

Atomic ~1.7x faster. All threads hit the same field — coarse and fine face the same contention, so they tie. CAS beats locking.

### Scattered

```
2. Scattered/Coarse (1 Mutex)
  time:   [85.225 µs 87.053 µs 89.724 µs]
  allocs: 712 bytes / 15 allocs

2. Scattered/Fine (3 Mutexes)
  time:   [89.395 µs 90.213 µs 91.199 µs]
  allocs: 736 bytes / 15 allocs

2. Scattered/Atomic
  time:   [52.232 µs 53.357 µs 54.744 µs]
  allocs: 640 bytes / 14 allocs
```

Atomic ~1.6x faster. Coarse serializes all threads even though they want different fields — worst-case for coarse.

### All Fields

```
3. All Fields/Coarse (1 Mutex)
  time:   [88.295 µs 88.712 µs 89.178 µs]
  allocs: 712 bytes / 15 allocs

3. All Fields/Fine (3 Mutexes)
  time:   [199.49 µs 201.15 µs 202.97 µs]
  allocs: 736 bytes / 15 allocs

3. All Fields/Atomic
  time:   [59.927 µs 60.384 µs 60.864 µs]
  allocs: 640 bytes / 14 allocs
```

Coarse locks once and does three writes — ~2.2x faster than Fine's 3 lock/unlock cycles. Atomic fastest with 3 independent CAS ops.

## Run

```sh
cargo bench --bench locking_granularity
```

## Verdict

**Coarse** if you always need everything atomically and don't want to think about it. **Fine** if fields get accessed independently — but you pay for it when touching everything. **Atomic** when you can get away with single-field ops and want maximum throughput.
