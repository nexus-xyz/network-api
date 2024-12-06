use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

// Distribution percentage (0.0 to 1.0) for the cancer-diagnostic program
// Example: 0.01 = 1%, 0.5 = 50%, 1.0 = 100%
const CANCER_DIAGNOSTIC_PERCENTAGE: f32 = 0.50;

pub fn get_program_for_prover(prover_id: &str) -> String {
    // Create a deterministic hash from the prover_id
    let mut hasher = DefaultHasher::new();
    prover_id.hash(&mut hasher);
    let deterministic_hash = hasher.finish();

    // Convert percentage (e.g., 0.01) to a number between 0-99
    // Example: 0.01 -> 1, 0.5 -> 50
    let percentage_of_cancer_program_in_programs = (CANCER_DIAGNOSTIC_PERCENTAGE * 100.0) as u64;

    // If hash mod 100 falls below percentage, select cancer-diagnostic
    let program_name = if deterministic_hash % 100 < percentage_of_cancer_program_in_programs {
        "cancer-diagnostic" // Selected with CANCER_DIAGNOSTIC_PERCENTAGE chance
    } else {
        "fast-fib" // Selected for remaining percentage
    };

    program_name.to_string()
}
