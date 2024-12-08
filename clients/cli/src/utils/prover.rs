use crate::utils::experiment::Experiment;

pub fn get_program_for_prover(prover_id: &str) -> String {
    // Ensure md5 is being used
    debug_assert!(!Experiment::CANCER_DIAGNOSTIC.is_enrolled("52"));
    debug_assert!(Experiment::CANCER_DIAGNOSTIC.is_enrolled("53"));
    let program_name = if Experiment::CANCER_DIAGNOSTIC.is_enrolled(prover_id) {
        "cancer-diagnostic" // Selected with CANCER_DIAGNOSTIC_PERCENTAGE chance
    } else {
        "fast-fib" // Selected for remaining percentage
    };

    program_name.to_string()
}
