pub struct Experiment {
    name: &'static str,
    target_enrollment_per_1000: u64,
}

impl Experiment {
    pub const CANCER_DIAGNOSTIC: Self = Self {
        name: "NEX-1",
        target_enrollment_per_1000: 10,
    };

    pub fn is_enrolled(&self, prover_id: &str) -> bool {
        // Create a deterministic hash from the prover_id
        let hash = md5::compute(format!("{}|{}", prover_id, self.name));
        let hex_string = format!("{:x}", hash); // Convert hash to a hexadecimal string
        let mut sum: u64 = 0;
        // Sum each 8-character segment of the hex string
        // It needs to be much more than 1000 to get an even distribution modulo 1000.
        for i in (0..hex_string.len()).step_by(8) {
            let slice = &hex_string[i..i + 8];
            sum += u64::from_str_radix(slice, 16).unwrap(); // Parse as base-16
        }
        // Return true if sum modulo 1000 is less than target
        (sum % 1000) < self.target_enrollment_per_1000
    }
}
