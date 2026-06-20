use criterion::{Criterion, criterion_group, criterion_main};
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

#[global_allocator]
static GLOBAL: &StatsAlloc<std::alloc::System> = &INSTRUMENTED_SYSTEM;

const NUM_THREADS: usize = 4;

// ═══════════════════════════════════════════════════════════════════════
// DATA STRUCTURES
// ═══════════════════════════════════════════════════════════════════════

// ── Shared by coarse & fine ──
struct Metrics {
    a: i32,
    b: i32,
    c: i32,
}

// ── Coarse: one Mutex over the whole struct ──
struct Coarse {
    lock: Mutex<Metrics>,
}

// ── Fine: per-field Mutexes ──
struct Fine {
    a: Mutex<i32>,
    b: Mutex<i32>,
    c: Mutex<i32>,
}

// ── Atomic: per-field AtomicI32, no locks ──
struct AtomicFields {
    a: AtomicI32,
    b: AtomicI32,
    c: AtomicI32,
}

// ═══════════════════════════════════════════════════════════════════════
// WORKLOAD: HOT FIELD
// all threads hammer field_a → max contention on same cacheline
// ═══════════════════════════════════════════════════════════════════════

fn coarse_hot(threads: usize, iters: usize) {
    let data = Arc::new(Coarse { lock: Mutex::new(Metrics { a: 0, b: 0, c: 0 }) });
    let mut hs = Vec::with_capacity(threads);
    for _ in 0..threads {
        let d = Arc::clone(&data);
        hs.push(thread::spawn(move || {
            for _ in 0..iters {
                d.lock.lock().unwrap().a += 1;
            }
        }));
    }
    for h in hs { h.join().unwrap(); }
}

fn fine_hot(threads: usize, iters: usize) {
    let data = Arc::new(Fine {
        a: Mutex::new(0), b: Mutex::new(0), c: Mutex::new(0),
    });
    let mut hs = Vec::with_capacity(threads);
    for _ in 0..threads {
        let d = Arc::clone(&data);
        hs.push(thread::spawn(move || {
            for _ in 0..iters {
                *d.a.lock().unwrap() += 1;
            }
        }));
    }
    for h in hs { h.join().unwrap(); }
}

fn atomic_hot(threads: usize, iters: usize) {
    let data = Arc::new(AtomicFields {
        a: AtomicI32::new(0), b: AtomicI32::new(0), c: AtomicI32::new(0),
    });
    let mut hs = Vec::with_capacity(threads);
    for _ in 0..threads {
        let d = Arc::clone(&data);
        hs.push(thread::spawn(move || {
            for _ in 0..iters {
                d.a.fetch_add(1, Ordering::Relaxed);
            }
        }));
    }
    for h in hs { h.join().unwrap(); }
}

// ═══════════════════════════════════════════════════════════════════════
// WORKLOAD: SCATTERED
// each thread targets a different field (t0→a, t1→b, t2→c, t3→a)
// coarse serializes everything; fine & atomic only contend on a
// ═══════════════════════════════════════════════════════════════════════

fn coarse_scattered(threads: usize, iters: usize) {
    let data = Arc::new(Coarse { lock: Mutex::new(Metrics { a: 0, b: 0, c: 0 }) });
    let mut hs = Vec::with_capacity(threads);
    for i in 0..threads {
        let d = Arc::clone(&data);
        hs.push(thread::spawn(move || {
            for _ in 0..iters {
                let mut g = d.lock.lock().unwrap();
                match i % 3 {
                    0 => g.a += 1,
                    1 => g.b += 1,
                    _ => g.c += 1,
                }
            }
        }));
    }
    for h in hs { h.join().unwrap(); }
}

fn fine_scattered(threads: usize, iters: usize) {
    let data = Arc::new(Fine {
        a: Mutex::new(0), b: Mutex::new(0), c: Mutex::new(0),
    });
    let mut hs = Vec::with_capacity(threads);
    for i in 0..threads {
        let d = Arc::clone(&data);
        hs.push(thread::spawn(move || {
            for _ in 0..iters {
                match i % 3 {
                    0 => *d.a.lock().unwrap() += 1,
                    1 => *d.b.lock().unwrap() += 1,
                    _ => *d.c.lock().unwrap() += 1,
                }
            }
        }));
    }
    for h in hs { h.join().unwrap(); }
}

fn atomic_scattered(threads: usize, iters: usize) {
    let data = Arc::new(AtomicFields {
        a: AtomicI32::new(0), b: AtomicI32::new(0), c: AtomicI32::new(0),
    });
    let mut hs = Vec::with_capacity(threads);
    for i in 0..threads {
        let d = Arc::clone(&data);
        hs.push(thread::spawn(move || {
            for _ in 0..iters {
                match i % 3 {
                    0 => { d.a.fetch_add(1, Ordering::Relaxed); }
                    1 => { d.b.fetch_add(1, Ordering::Relaxed); }
                    _ => { d.c.fetch_add(1, Ordering::Relaxed); }
                }
            }
        }));
    }
    for h in hs { h.join().unwrap(); }
}

// ═══════════════════════════════════════════════════════════════════════
// WORKLOAD: ALL FIELDS
// each thread increments all 3 fields in sequence
// coarse = 1 lock/unlock; fine = 3 lock/unlock; atomic = 3 CAS ops
// ═══════════════════════════════════════════════════════════════════════

fn coarse_all(threads: usize, iters: usize) {
    let data = Arc::new(Coarse { lock: Mutex::new(Metrics { a: 0, b: 0, c: 0 }) });
    let mut hs = Vec::with_capacity(threads);
    for _ in 0..threads {
        let d = Arc::clone(&data);
        hs.push(thread::spawn(move || {
            for _ in 0..iters {
                let mut g = d.lock.lock().unwrap();
                g.a += 1;
                g.b += 1;
                g.c += 1;
            }
        }));
    }
    for h in hs { h.join().unwrap(); }
}

fn fine_all(threads: usize, iters: usize) {
    let data = Arc::new(Fine {
        a: Mutex::new(0), b: Mutex::new(0), c: Mutex::new(0),
    });
    let mut hs = Vec::with_capacity(threads);
    for _ in 0..threads {
        let d = Arc::clone(&data);
        hs.push(thread::spawn(move || {
            for _ in 0..iters {
                *d.a.lock().unwrap() += 1;
                *d.b.lock().unwrap() += 1;
                *d.c.lock().unwrap() += 1;
            }
        }));
    }
    for h in hs { h.join().unwrap(); }
}

fn atomic_all(threads: usize, iters: usize) {
    let data = Arc::new(AtomicFields {
        a: AtomicI32::new(0), b: AtomicI32::new(0), c: AtomicI32::new(0),
    });
    let mut hs = Vec::with_capacity(threads);
    for _ in 0..threads {
        let d = Arc::clone(&data);
        hs.push(thread::spawn(move || {
            for _ in 0..iters {
                d.a.fetch_add(1, Ordering::Relaxed);
                d.b.fetch_add(1, Ordering::Relaxed);
                d.c.fetch_add(1, Ordering::Relaxed);
            }
        }));
    }
    for h in hs { h.join().unwrap(); }
}

// ═══════════════════════════════════════════════════════════════════════
// MEMORY PROFILING
// ═══════════════════════════════════════════════════════════════════════

fn profile_memory() {
    println!("\n==================================================");
    println!("          HEAP ALLOCATION PROFILING               ");
    println!("==================================================");

    // ── Coarse ──
    let reg = Region::new(&GLOBAL);
    coarse_hot(NUM_THREADS, 10);
    let s = reg.change();
    println!("Coarse (1 Mutex):");
    println!("  {} bytes / {} allocs", s.bytes_allocated, s.allocations);

    // ── Fine ──
    let reg = Region::new(&GLOBAL);
    fine_hot(NUM_THREADS, 10);
    let s = reg.change();
    println!("\nFine (3 Mutexes):");
    println!("  {} bytes / {} allocs", s.bytes_allocated, s.allocations);

    // ── Atomic ──
    let reg = Region::new(&GLOBAL);
    atomic_hot(NUM_THREADS, 10);
    let s = reg.change();
    println!("\nAtomic (no locks):");
    println!("  {} bytes / {} allocs", s.bytes_allocated, s.allocations);

    println!("==================================================\n");
}

// ═══════════════════════════════════════════════════════════════════════
// CRITERION ORCHESTRATION
// ═══════════════════════════════════════════════════════════════════════

fn compare_locking(c: &mut Criterion) {
    profile_memory();

    // ── Hot Field: all threads hammer field a ──
    let mut g = c.benchmark_group("1. Hot Field");
    g.bench_function("Coarse (1 Mutex)", |b| b.iter(|| coarse_hot(NUM_THREADS, 1000)));
    g.bench_function("Fine (3 Mutexes)", |b| b.iter(|| fine_hot(NUM_THREADS, 1000)));
    g.bench_function("Atomic", |b| b.iter(|| atomic_hot(NUM_THREADS, 1000)));
    g.finish();

    // ── Scattered: each thread targets a different field ──
    let mut g = c.benchmark_group("2. Scattered");
    g.bench_function("Coarse (1 Mutex)", |b| b.iter(|| coarse_scattered(NUM_THREADS, 1000)));
    g.bench_function("Fine (3 Mutexes)", |b| b.iter(|| fine_scattered(NUM_THREADS, 1000)));
    g.bench_function("Atomic", |b| b.iter(|| atomic_scattered(NUM_THREADS, 1000)));
    g.finish();

    // ── All Fields: each thread updates all 3 fields per iter ──
    let mut g = c.benchmark_group("3. All Fields");
    g.bench_function("Coarse (1 Mutex)", |b| b.iter(|| coarse_all(NUM_THREADS, 1000)));
    g.bench_function("Fine (3 Mutexes)", |b| b.iter(|| fine_all(NUM_THREADS, 1000)));
    g.bench_function("Atomic", |b| b.iter(|| atomic_all(NUM_THREADS, 1000)));
    g.finish();
}

criterion_group!(benches, compare_locking);
criterion_main!(benches);
