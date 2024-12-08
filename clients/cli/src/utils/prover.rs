use crate::utils::experiment::Experiment;

pub fn get_program_for_prover(prover_id: &str) -> String {
    // For an experiment with 1% enrollment, the first 53 provers
    // are not enrolled and then we get an enrolled one, when
    // md5 is being used. A few assertions here ensure that the
    // algorithm is producing results that match web.
    debug_assert!(!Experiment::CANCER_DIAGNOSTIC.is_enrolled("52"));
    debug_assert!(Experiment::CANCER_DIAGNOSTIC.is_enrolled("53"));
    let program_name = if Experiment::CANCER_DIAGNOSTIC.is_enrolled(prover_id) {
        "cancer-diagnostic" // Selected with CANCER_DIAGNOSTIC_PERCENTAGE chance
    } else {
        "fast-fib" // Selected for remaining percentage
    };

    program_name.to_string()
}
