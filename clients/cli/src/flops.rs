// Measure the FLOPS of the CPU
// use num_cpus;
use rayon::prelude::*;
use std::time::Instant;

const NTESTS: u64 = 1_000_000;
const OPERATIONS_PER_ITERATION: u64 = 4; // sin, add, multiply, divide

pub fn measure_flops() -> f32 {
    let num_cores = num_cpus::get() as u64;
    let start = Instant::now();

    let total_flops: f64 = (0..num_cores)
        .into_par_iter()
        .map(|_| {
            let mut x: f64 = 1.0;
            for _ in 0..NTESTS {
                x = (x.sin() + 1.0) * 0.5 / 1.1;
            }
            // Prevent compiler from optimizing out the loop
            if x.is_nan() {
                println!("This should never happen");
            }
            NTESTS * OPERATIONS_PER_ITERATION
        })
        .sum::<u64>() as f64;

    let duration = start.elapsed();

    let flops = total_flops / duration.as_secs_f64();
    flops as f32
}
