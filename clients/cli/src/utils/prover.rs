use crate::utils::experiment::Experiment;

pub fn get_program_for_prover(prover_id: &str) -> String {
    // If hash mod 100 falls below percentage, select cancer-diagnostic
    let program_name = if Experiment::CANCER_DIAGNOSTIC.is_enrolled(prover_id) {
        "cancer-diagnostic" // Selected with CANCER_DIAGNOSTIC_PERCENTAGE chance
    } else {
        "fast-fib" // Selected for remaining percentage
    };

    program_name.to_string()
}
