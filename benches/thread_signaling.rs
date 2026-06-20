use criterion::{Criterion, criterion_group, criterion_main};
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread;

// 1. Link the tracking allocator globally
#[global_allocator]
static GLOBAL: &StatsAlloc<std::alloc::System> = &INSTRUMENTED_SYSTEM;

// --- STRATEGY 1: CONDVAR ---
struct WorkState {
    is_finished: bool,
}
fn bench_condvar() {
    let pair = Arc::new((Mutex::new(WorkState { is_finished: false }), Condvar::new()));
    let pair_clone = Arc::clone(&pair);

    thread::scope(|s| {
        s.spawn(move || {
            let (lock, cvar) = &*pair_clone;
            let mut state = lock.lock().unwrap();
            state.is_finished = true;
            cvar.notify_one();
        });

        let (lock, cvar) = &*pair;
        let mut state = lock.lock().unwrap();
        while !state.is_finished {
            state = cvar.wait(state).unwrap();
        }
    });
}

// --- STRATEGY 2: CHANNELS ---
fn bench_channels() {
    let (tx, rx) = mpsc::channel();

    thread::scope(|s| {
        s.spawn(move || {
            tx.send(()).unwrap();
        });

        let _ = rx.recv().unwrap();
    });
}

// --- STRATEGY 3: THREAD PARK ---
fn bench_parking() {
    let main_thread_handle = thread::current();

    thread::scope(|s| {
        s.spawn(move || {
            main_thread_handle.unpark();
        });

        thread::park();
    });
}

// --- PROFILE MEMORY FUNCTION ---
// Runs each function exactly once in a pristine allocation region to print heap metrics
fn profile_memory() {
    println!("\n==================================================");
    println!("          HEAP ALLOCATION PROFILING               ");
    println!("==================================================");

    // Profile Condvar
    let reg = Region::new(&GLOBAL);
    bench_condvar();
    let stats = reg.change();
    println!("Condvar (Mutex):");
    println!(
        "  Allocated:   {} bytes across {} allocations",
        stats.bytes_allocated, stats.allocations
    );

    // Profile Channels
    let reg = Region::new(&GLOBAL);
    bench_channels();
    let stats = reg.change();
    println!("\nMPSC Channel:");
    println!(
        "  Allocated:   {} bytes across {} allocations",
        stats.bytes_allocated, stats.allocations
    );

    // Profile Thread Parking
    let reg = Region::new(&GLOBAL);
    bench_parking();
    let stats = reg.change();
    println!("\nThread Parking:");
    println!(
        "  Allocated:   {} bytes across {} allocations",
        stats.bytes_allocated, stats.allocations
    );
    println!("==================================================\n");
}

// --- CRITERION ORCHESTRATION ---
fn compare_signaling(c: &mut Criterion) {
    // Run the pure heap profile calculation right before or after Criterion execution
    profile_memory();

    let mut group = c.benchmark_group("Thread Signaling");

    group.bench_function("Condvar (Mutex)", |b| b.iter(|| bench_condvar()));
    group.bench_function("MPSC Channel", |b| b.iter(|| bench_channels()));
    group.bench_function("Thread Parking", |b| b.iter(|| bench_parking()));

    group.finish();
}

criterion_group!(benches, compare_signaling);
criterion_main!(benches);
