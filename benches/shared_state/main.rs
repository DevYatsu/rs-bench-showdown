use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};
use std::sync::{Arc, LazyLock, Mutex, OnceLock};
use std::thread;

#[global_allocator]
static GLOBAL: &StatsAlloc<std::alloc::System> = &INSTRUMENTED_SYSTEM;

const NUM_THREADS: usize = 4;

// ═══════════════════════════════════════════════════════════════════════
// READ-ONLY BENCHMARKS (no Mutex — share &T directly)
// Difference: how the &T ref reaches each thread
// ═══════════════════════════════════════════════════════════════════════

// ── 1. Arc<T> ──
// Heap-allocated, refcounted. Clone costs one atomic increment.
fn arc_read(threads: usize, reads: usize) {
    let data = Arc::new(100i64);
    let mut hs = Vec::with_capacity(threads);
    for _ in 0..threads {
        let d = Arc::clone(&data);
        hs.push(thread::spawn(move || {
            let mut s = 0i64;
            for _ in 0..reads {
                s += black_box(*d);
            }
            black_box(s);
        }));
    }
    for h in hs {
        h.join().unwrap();
    }
}

// ── 2. Static LazyLock<T> ──
// Global singleton — initialized once on first access, never cleaned up.
static LAZY_VAL: LazyLock<i64> = LazyLock::new(|| 100);

fn lazy_read(threads: usize, reads: usize) {
    LazyLock::force(&LAZY_VAL);
    let mut hs = Vec::with_capacity(threads);
    for _ in 0..threads {
        hs.push(thread::spawn(move || {
            let mut s = 0i64;
            for _ in 0..reads {
                s += black_box(*LAZY_VAL);
            }
            black_box(s);
        }));
    }
    for h in hs {
        h.join().unwrap();
    }
}

// ── 3. Box::leak → &'static T ──
// Leak a Box into eternity. No refcounting, no lazy-init overhead.
static LEAKED_VAL: OnceLock<&i64> = OnceLock::new();

fn get_leaked_val() -> &'static i64 {
    LEAKED_VAL.get_or_init(|| Box::leak(Box::new(100)))
}

fn leak_read(threads: usize, reads: usize) {
    let val = get_leaked_val();
    let mut hs = Vec::with_capacity(threads);
    for _ in 0..threads {
        hs.push(thread::spawn(move || {
            let mut s = 0i64;
            for _ in 0..reads {
                s += black_box(*val);
            }
            black_box(s);
        }));
    }
    for h in hs {
        h.join().unwrap();
    }
}

// ── 4. thread::scope ──
// Just borrow a local. Zero alloc. No 'static needed. Auto-joins on scope exit.
fn scope_read(threads: usize, reads: usize) {
    let val = 100i64;
    thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                let mut sum = 0i64;
                for _ in 0..reads {
                    sum += black_box(val);
                }
                black_box(sum);
            });
        }
    });
}

// ═══════════════════════════════════════════════════════════════════════
// READ+WRITE BENCHMARKS (with Mutex — share &Mutex<T>)
// Same as read-only, but now with lock contention added
// ═══════════════════════════════════════════════════════════════════════

// ── 1. Arc<Mutex<T>> ──
fn arc_write(threads: usize, iters: usize) {
    let data = Arc::new(Mutex::new(0i64));
    let mut hs = Vec::with_capacity(threads);
    for _ in 0..threads {
        let d = Arc::clone(&data);
        hs.push(thread::spawn(move || {
            for _ in 0..iters {
                *d.lock().unwrap() += 1;
            }
        }));
    }
    for h in hs {
        h.join().unwrap();
    }
}

// ── 2. Static LazyLock<Mutex<T>> ──
static SHARED_LAZY: LazyLock<Mutex<i64>> = LazyLock::new(|| Mutex::new(0));

fn lazy_write(threads: usize, iters: usize) {
    LazyLock::force(&SHARED_LAZY);
    let mut hs = Vec::with_capacity(threads);
    for _ in 0..threads {
        hs.push(thread::spawn(move || {
            for _ in 0..iters {
                *SHARED_LAZY.lock().unwrap() += 1;
            }
        }));
    }
    for h in hs {
        h.join().unwrap();
    }
}

// ── 3. Box::leak → &'static Mutex<T> ──
static SHARED_LEAK: OnceLock<&Mutex<i64>> = OnceLock::new();

fn get_leaked_mutex() -> &'static Mutex<i64> {
    SHARED_LEAK.get_or_init(|| Box::leak(Box::new(Mutex::new(0))))
}

fn leak_write(threads: usize, iters: usize) {
    let data = get_leaked_mutex();
    let mut hs = Vec::with_capacity(threads);
    for _ in 0..threads {
        hs.push(thread::spawn(move || {
            for _ in 0..iters {
                *data.lock().unwrap() += 1;
            }
        }));
    }
    for h in hs {
        h.join().unwrap();
    }
}

// ── 4. thread::scope ──
fn scope_write(threads: usize, iters: usize) {
    let data = Mutex::new(0i64);
    thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                for _ in 0..iters {
                    *data.lock().unwrap() += 1;
                }
            });
        }
    });
}

// ═══════════════════════════════════════════════════════════════════════
// MEMORY PROFILING
// ═══════════════════════════════════════════════════════════════════════

fn profile_memory() {
    println!("\n==================================================");
    println!("          HEAP ALLOCATION PROFILING               ");
    println!("==================================================");

    println!("── Read-Only ──");

    let reg = Region::new(&GLOBAL);
    arc_read(NUM_THREADS, 10);
    let s = reg.change();
    println!(
        "Arc<T>:\n  Allocated: {} bytes / {} allocs",
        s.bytes_allocated, s.allocations
    );

    let reg = Region::new(&GLOBAL);
    lazy_read(NUM_THREADS, 10);
    let s = reg.change();
    println!(
        "\nLazyLock<T>:\n  Allocated: {} bytes / {} allocs",
        s.bytes_allocated, s.allocations
    );

    let reg = Region::new(&GLOBAL);
    leak_read(NUM_THREADS, 10);
    let s = reg.change();
    println!(
        "\nBox::leak:\n  Allocated: {} bytes / {} allocs",
        s.bytes_allocated, s.allocations
    );

    let reg = Region::new(&GLOBAL);
    scope_read(NUM_THREADS, 10);
    let s = reg.change();
    println!(
        "\nthread::scope:\n  Allocated: {} bytes / {} allocs",
        s.bytes_allocated, s.allocations
    );

    println!("\n── Read+Write ──");

    let reg = Region::new(&GLOBAL);
    arc_write(NUM_THREADS, 10);
    let s = reg.change();
    println!(
        "Arc<Mutex<T>>:\n  Allocated: {} bytes / {} allocs",
        s.bytes_allocated, s.allocations
    );

    let reg = Region::new(&GLOBAL);
    lazy_write(NUM_THREADS, 10);
    let s = reg.change();
    println!(
        "\nLazyLock<Mutex<T>>:\n  Allocated: {} bytes / {} allocs",
        s.bytes_allocated, s.allocations
    );

    let reg = Region::new(&GLOBAL);
    leak_write(NUM_THREADS, 10);
    let s = reg.change();
    println!(
        "\nBox::leak:\n  Allocated: {} bytes / {} allocs",
        s.bytes_allocated, s.allocations
    );

    let reg = Region::new(&GLOBAL);
    scope_write(NUM_THREADS, 10);
    let s = reg.change();
    println!(
        "\nthread::scope:\n  Allocated: {} bytes / {} allocs",
        s.bytes_allocated, s.allocations
    );

    println!("==================================================\n");
}

// ═══════════════════════════════════════════════════════════════════════
// CRITERION ORCHESTRATION
// ═══════════════════════════════════════════════════════════════════════

fn compare_shared_state(c: &mut Criterion) {
    profile_memory();

    // ── Read-Only: 1000 reads/thread ──
    let mut g = c.benchmark_group("1. Read-Only (1000 reads/thread)");
    g.bench_function("Arc<T>", |b| b.iter(|| arc_read(NUM_THREADS, 1000)));
    g.bench_function("LazyLock<T>", |b| b.iter(|| lazy_read(NUM_THREADS, 1000)));
    g.bench_function("Box::leak", |b| b.iter(|| leak_read(NUM_THREADS, 1000)));
    g.bench_function("thread::scope", |b| b.iter(|| scope_read(NUM_THREADS, 1000)));
    g.finish();

    // ── Read+Write: 1000 writes/thread ──
    let mut g = c.benchmark_group("2. Read+Write (1000 writes/thread)");
    g.bench_function("Arc<Mutex<T>>", |b| b.iter(|| arc_write(NUM_THREADS, 1000)));
    g.bench_function("LazyLock<Mutex<T>>", |b| b.iter(|| lazy_write(NUM_THREADS, 1000)));
    g.bench_function("Box::leak", |b| b.iter(|| leak_write(NUM_THREADS, 1000)));
    g.bench_function("thread::scope", |b| b.iter(|| scope_write(NUM_THREADS, 1000)));
    g.finish();
}

criterion_group!(benches, compare_shared_state);
criterion_main!(benches);
