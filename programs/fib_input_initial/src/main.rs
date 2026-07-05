#![cfg_attr(target_arch = "riscv32", no_std, no_main)]
#[cfg(target_arch = "riscv32")]
use nexus_rt::println;
#[cfg(not(target_arch = "riscv32"))]
use std::println;

#[cfg(not(target_arch = "riscv32"))]
fn public_input_native() -> Result<(u32, u32, u32), String> {
    use std::io::{self, BufRead};
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();

    let line1 = lines
        .next()
        .ok_or("No first input provided")?
        .map_err(|e| format!("Failed to read first line: {}", e))?;

    let n = line1
        .trim()
        .parse()
        .map_err(|e| format!("Failed to parse first input as u32: {}", e))?;

    let init_a = match lines.next() {
        Some(Ok(line)) => line.trim().parse().unwrap_or(1),
        _ => 1,
    };

    let init_b = match lines.next() {
        Some(Ok(line)) => line.trim().parse().unwrap_or(1),
        _ => 1,
    };

    Ok((n, init_a, init_b))
}

#[nexus_rt::main]
#[cfg_attr(target_arch = "riscv32", nexus_rt::public_input(n, init_a, init_b))]
#[cfg_attr(
    not(target_arch = "riscv32"),
    nexus_rt::custom_input((n, init_a, init_b), public_input_native)
)]
fn main(n: u32, init_a: u32, init_b: u32) {
    // Simple Fibonacci calculation
    let mut prev: u32 = init_a;
    let mut curr: u32 = init_b;

    for _ in 0..n {
        let next = prev.wrapping_add(curr);
        prev = curr;
        curr = next;
    }

    println!("{:?}", curr);
}
