// The following enum is used to determine the environment from the web socket string
#[derive(Debug, Clone)]
pub enum Environment {
    Local,
    Dev,
    Staging,
    Beta,
}

impl Environment {
    pub fn orchestrator_url(&self) -> String {
        match self {
            Environment::Local => "http://localhost:8080".to_string(),
            Environment::Dev => "https://dev.orchestrator.nexus.xyz".to_string(),
            Environment::Staging => "https://staging.orchestrator.nexus.xyz".to_string(),
            Environment::Beta => "https://beta.orchestrator.nexus.xyz".to_string(),
        }
    }

    pub fn from_args(env: Option<&crate::Environment>) -> Self {
        match env {
            Some(crate::Environment::Local) => Environment::Local,
            Some(crate::Environment::Dev) => Environment::Dev,
            Some(crate::Environment::Staging) => Environment::Staging,
            Some(crate::Environment::Beta) => Environment::Beta,
            None => Environment::Local, // Default
        }
    }
}

impl std::fmt::Display for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Environment::Local => write!(f, "Local"),
            Environment::Dev => write!(f, "Development"),
            Environment::Staging => write!(f, "Staging"),
            Environment::Beta => write!(f, "Beta"),
        }
    }
}

mod analytics {
    pub const DEV_MEASUREMENT_ID: &str = "G-SWNG3LZDFR";
    pub const STAGING_MEASUREMENT_ID: &str = "G-T0M0Q3V6WN";
    pub const BETA_MEASUREMENT_ID: &str = "G-GLH0GMEEFH";
    pub const DEV_API_SECRET: &str = "8ySxiKrtT8a76zClqqO8IQ";
    pub const STAGING_API_SECRET: &str = "OI7H53soRMSDWfJf1ittHQ";
    pub const BETA_API_SECRET: &str = "3wxu8FjVSPqOlxSsZEnBOw";
}

pub fn analytics_id(environment: &Environment) -> String {
    match environment {
        Environment::Dev => analytics::DEV_MEASUREMENT_ID.to_string(),
        Environment::Staging => analytics::STAGING_MEASUREMENT_ID.to_string(),
        Environment::Beta => analytics::BETA_MEASUREMENT_ID.to_string(),
        Environment::Local => String::new(),
    }
}

pub fn analytics_api_key(environment: &Environment) -> String {
    match environment {
        Environment::Dev => analytics::DEV_API_SECRET.to_string(),
        Environment::Staging => analytics::STAGING_API_SECRET.to_string(),
        Environment::Beta => analytics::BETA_API_SECRET.to_string(),
        Environment::Local => String::new(),
    }
}
