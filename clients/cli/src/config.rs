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

    pub fn from_args(local: bool, dev: bool, staging: bool, beta: bool) -> Self {
        if local {
            Environment::Local
        } else if dev {
            Environment::Dev
        } else if beta {
            Environment::Beta
        } else if staging {
            Environment::Staging
        } else {
            // Default to staging when no flag is set
            Environment::Staging
        }
    }
}
