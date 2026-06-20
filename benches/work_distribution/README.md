# Work Distribution

Manual `thread::scope` with static chunking vs Rayon work-stealing, on two workload shapes.

## Workloads

| Workload | Behavior | Problem |
|---|---|---|
| **Uniform** | `*item += 10` per element | Predictable, same cost per item. No load imbalance. |
| **Non-Uniform** | If index % 7 == 0: 50x loop, else: add 1 | Branch bottleneck. Every 7th item ~50x heavier. Static chunks get unlucky. |

## Approaches

| Approach | Allocs | Tradeoff |
|---|---|---|
| **Manual `thread::scope` + `chunks_mut`** | 552 bytes / 13 allocs (static ref chunks) | Zero overhead. Pre-divided chunks — if workload spiky, one thread starves while others idle. |
| **Rayon `par_iter_mut`** | 63480 bytes / 116 allocs (threadpool setup) | Work-stealing scheduler: idle threads steal from busy ones. Auto-tuned to core count. Heavier setup, but adapts to imbalance. |

## Results (Apple M3, 8 cores, 16 GB)

```
1. Uniform Workload/Base Scope Chunks
  time:   [1.5054 ms 1.5897 ms 1.6763 ms]

1. Uniform Workload/Rayon par_iter
  time:   [2.9972 ms 3.4299 ms 3.9651 ms]

2. Non-Uniform Workload/Base Scope Chunks
  time:   [7.5621 ms 7.8265 ms 8.1040 ms]

2. Non-Uniform Workload/Rayon par_iter
  time:   [5.2163 ms 5.3629 ms 5.5264 ms]
```

- **Uniform:** Manual chunks ~2x faster — no work-stealing overhead.
- **Non-uniform:** Rayon ~30% faster — work-stealing redistributes spikes.

## Run

```sh
cargo bench --bench work_distribution
```

## Verdict

Manual chunks if your work is predictable. Rayon if you've got spikes — it's what work-stealing was built for.
