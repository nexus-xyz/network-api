const CANCER_DIAGNOSTIC_PROBABILITY: f32 = 0.01;

pub fn get_random_program() -> String {
    // There are two programs to choose from, with some % chance of using the cancer diagnostic program
    let programs = ["fast-fib", "cancer-diagnostic"];

    let program_name = if rand::random::<f32>() < CANCER_DIAGNOSTIC_PROBABILITY {
        programs[1]
    } else {
        programs[0]
    };

    format!("src/generated/{}", program_name)
}
