use criterion::{Criterion, criterion_group, criterion_main};
use rayon::prelude::*;
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};
use std::thread;

#[global_allocator]
static GLOBAL: &StatsAlloc<std::alloc::System> = &INSTRUMENTED_SYSTEM;

// =========================================================================
// UNIFORM: every element gets the same cheap op
// =========================================================================

fn uniform_base_impl(data: &mut [i32], num_threads: usize) {
    let chunk_size = (data.len() + num_threads - 1) / num_threads;

    thread::scope(|s| {
        for chunk in data.chunks_mut(chunk_size) {
            s.spawn(move || {
                for item in chunk.iter_mut() {
                    *item += 10;
                }
            });
        }
    });
}

fn uniform_rayon_impl(data: &mut [i32]) {
    data.par_iter_mut().for_each(|item| {
        *item += 10;
    });
}

// =========================================================================
// NON-UNIFORM: some elements way heavier (index % 7 hits a 50-iteration loop)
// =========================================================================

#[inline(never)]
fn heavy_computation(index: usize, val: i32) -> i32 {
    // Every 7th index gets a 50-iteration loop — creates load imbalance
    if index % 7 == 0 {
        let mut temp = val;
        for i in 0..50 {
            temp = temp.wrapping_add(i);
        }
        temp
    } else {
        val.wrapping_add(1)
    }
}

fn non_uniform_base_impl(data: &mut [i32], num_threads: usize) {
    let chunk_size = (data.len() + num_threads - 1) / num_threads;

    thread::scope(|s| {
        // Map chunk index back to global element index
        for (chunk_idx, chunk) in data.chunks_mut(chunk_size).enumerate() {
            s.spawn(move || {
                let start_idx = chunk_idx * chunk_size;
                for (offset, item) in chunk.iter_mut().enumerate() {
                    *item = heavy_computation(start_idx + offset, *item);
                }
            });
        }
    });
}

fn non_uniform_rayon_impl(data: &mut [i32]) {
    // .enumerate() works directly on Rayon's parallel iterators
    data.par_iter_mut().enumerate().for_each(|(index, item)| {
        *item = heavy_computation(index, *item);
    });
}

// =========================================================================
// ALLOCATION PROFILER
// =========================================================================

fn profile_memory(num_elements: usize, num_threads: usize) {
    println!("\n==================================================");
    println!("          HEAP ALLOCATION PROFILING               ");
    println!("==================================================");

    let mut data = vec![1; num_elements];

    // Profile Base
    let reg = Region::new(&GLOBAL);
    uniform_base_impl(&mut data, num_threads);
    let stats = reg.change();
    println!("Base Impl (Static Reference Chunks):");
    println!(
        "  Allocated:   {} bytes across {} allocations",
        stats.bytes_allocated, stats.allocations
    );

    // Profile Rayon
    let reg = Region::new(&GLOBAL);
    uniform_rayon_impl(&mut data);
    let stats = reg.change();
    println!("\nRayon Impl (Work-Stealing Iterator):");
    println!(
        "  Allocated:   {} bytes across {} allocations",
        stats.bytes_allocated, stats.allocations
    );
    println!("==================================================\n");
}

// =========================================================================
// CRITERION ORCHESTRATION
// =========================================================================

fn compare_work_distribution(c: &mut Criterion) {
    let num_elements = 10_000_000;
    let num_threads = 4;

    // Allocate the giant data vector
    let mut giant_array = vec![1; num_elements];

    profile_memory(num_elements, num_threads);

    // --- BENCHMARK GROUP 1: UNIFORM ---
    let mut group_uniform = c.benchmark_group("1. Uniform Workload");
    group_uniform.bench_function("Base Scope Chunks", |b| {
        b.iter(|| uniform_base_impl(&mut giant_array, num_threads))
    });
    group_uniform.bench_function("Rayon par_iter", |b| {
        b.iter(|| uniform_rayon_impl(&mut giant_array))
    });
    group_uniform.finish();

    // --- BENCHMARK GROUP 2: NON-UNIFORM ---
    let mut group_non_uniform = c.benchmark_group("2. Non-Uniform Workload");
    group_non_uniform.bench_function("Base Scope Chunks", |b| {
        b.iter(|| non_uniform_base_impl(&mut giant_array, num_threads))
    });
    group_non_uniform.bench_function("Rayon par_iter", |b| {
        b.iter(|| non_uniform_rayon_impl(&mut giant_array))
    });
    group_non_uniform.finish();
}

criterion_group!(benches, compare_work_distribution);
criterion_main!(benches);
