use rayon::prelude::*;
use std::hint::black_box;
use std::time::Instant;

const NTESTS: u64 = 1_000_000;
const OPERATIONS_PER_ITERATION: u64 = 4; // sin, add, multiply, divide
const NUM_REPEATS: usize = 5; // Number of repeats to average the results

pub fn measure_flops() -> f32 {
    let num_cores = num_cpus::get() as u64;
    println!("Using {} logical cores for FLOPS measurement", num_cores);

    let avg_flops: f64 = (0..NUM_REPEATS)
        .map(|_| {
            let start = Instant::now();

            let total_flops: u64 = (0..num_cores)
                .into_par_iter()
                .map(|_| {
                    let mut x: f64 = 1.0;
                    for _ in 0..NTESTS {
                        x = black_box((x.sin() + 1.0) * 0.5 / 1.1);
                    }
                    NTESTS * OPERATIONS_PER_ITERATION
                })
                .sum();

            total_flops as f64 / start.elapsed().as_secs_f64()
        })
        .sum::<f64>()
        / NUM_REPEATS as f64; // Average the FLOPS over all repeats

    (avg_flops / 1e9) as f32 // Convert to GFLOP/s and cast to f32
}
