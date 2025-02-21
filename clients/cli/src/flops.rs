use rayon::prelude::*;
use std::hint::black_box;
use std::time::Instant;

const NTESTS: u64 = 1_000_000;
const OPERATIONS_PER_ITERATION: u64 = 4; // sin, add, multiply, divide
const NUM_REPEATS: usize = 5; // Число повторов для усреднения

pub fn measure_flops() -> f64 {
    let num_threads = rayon::current_num_threads();
    
    let avg_flops: f64 = (0..NUM_REPEATS)
        .map(|_| {
            let start = Instant::now();

            let total_flops: u64 = (0..num_threads)
                .into_par_iter()
                .map(|_| {
                    let mut x: f64 = 1.0;
                    for _ in 0..NTESTS {
                        x = black_box((x.sin() + 1.0) * 0.5 / 1.1);
                    }
                    NTESTS * OPERATIONS_PER_ITERATION
                })
                .sum();

            let duration = start.elapsed().as_secs_f64();
            total_flops as f64 / duration
        })
        .sum::<f64>()
        / NUM_REPEATS as f64; // Усреднение результатов

    avg_flops
}

fn main() {
    let flops = measure_flops();
    println!("CPU FLOPS: {:.3} GFLOP/s", flops / 1e9);
}
