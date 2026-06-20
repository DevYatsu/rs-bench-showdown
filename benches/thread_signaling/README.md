# Thread Signaling

Three ways to signal one thread from another.

## Approaches

| Approach | Mechanism | Heap Allocs | Tradeoff |
|---|---|---|---|
| **Condvar (Mutex)** | `Mutex<bool>` + `Condvar::wait/notify_one` | Mutex + Condvar init per run | Cheapest once set up. Can carry shared state. But the API is verbose — forget the predicate loop and you're debugging spurious wakeups at 2am. |
| **MPSC Channel** | `mpsc::channel::<()>` + `send/recv` | 2 allocs (channel buf) | Simple API, channel can carry data. Heap alloc per-use. Fits multi-producer / single-consumer well. |
| **Thread Parking** | `thread::park/unpark` | 0 allocs | Zero allocation. Simplest code. No shared state — pure wakeup signal. Can lose wakeups if `unpark` fires before `park`. |

## Results (Apple M3, 8 cores, 16 GB)

```
Thread Signaling/Condvar (Mutex)
  time:   [11.717 µs 11.799 µs 11.872 µs]
  allocs: 320 bytes / 7 allocations

Thread Signaling/MPSC Channel
  time:   [12.583 µs 12.626 µs 12.665 µs]
  allocs: 1144 bytes / 9 allocations

Thread Signaling/Thread Parking
  time:   [11.480 µs 11.533 µs 11.601 µs]
  allocs: 160 bytes / 4 allocations
```

All three are within a couple microseconds. The difference is in allocation cost and API ergonomics.

## Run

```sh
cargo bench --bench thread_signaling
```

## Verdict

Use `thread::park` if you just need a wakeup — it's the fastest, zero alloc, simplest code. Use Condvar if you need shared state too. Use a Channel for multi-producer scenarios — you pay a small alloc for the convenience.
