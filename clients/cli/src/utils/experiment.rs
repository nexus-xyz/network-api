pub struct Experiment{ name: &'static str, target_enrollment_per_1000: u32 }

impl Experiment {
    pub const CANCER_DIAGNOSTIC: Self = Self{name: "NEX-1", target_enrollment_per_1000: 10};

    pub fn get_enrollment_for_prover(&self, prover_id: &str) -> bool {
        // Create a deterministic hash from the prover_id
        let digest = md5::compute(format!("{}|{}", prover_id, self.name));
        
        return false
    }
}
