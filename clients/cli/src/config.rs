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
        let url = match self {
            Environment::Local => "http://localhost:8080".to_string(),
            Environment::Dev => "https://dev.orchestrator.nexus.xyz".to_string(),
            Environment::Staging => "https://staging.orchestrator.nexus.xyz".to_string(),
            Environment::Beta => "https://beta.orchestrator.nexus.xyz".to_string(),
        };
        url
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

// // the firebase APP IDS by environment
// mod firebase {
//     pub const DEV_APP_ID: &str = "1:954530464230:web:f0a14de14ef7bcdaa99627";
//     pub const STAGING_APP_ID: &str = "1:222794630996:web:1758d64a85eba687eaaac1";
//     pub const BETA_APP_ID: &str = "1:279395003658:web:04ee2c524474d683d75ef3";

//     // Analytics keys for the different environments
//     // These are keys that allow the measurement protocol to write to the analytics database
//     // They are not sensitive. Worst case, if a malicious actor obtains the secret, they could potentially send false or misleading data to your GA4 property
//     pub const DEV_API_SECRET: &str = "8ySxiKrtT8a76zClqqO8IQ";
//     pub const STAGING_API_SECRET: &str = "OI7H53soRMSDWfJf1ittHQ";
//     pub const BETA_API_SECRET: &str = "gxxzKAQLSl-uYI0eKbIi_Q";
// }

// // Release versions (existing code)
// pub fn analytics_id(env: &Environment) -> String {
//     // Return the appropriate Firebase App ID based on the environment
//     match env {
//         Environment::Dev => firebase::DEV_APP_ID.to_string(),
//         Environment::Staging => firebase::STAGING_APP_ID.to_string(),
//         Environment::Beta => firebase::BETA_APP_ID.to_string(),
//         _ => String::new(),
//     }
// }

// pub fn analytics_api_key(env: &Environment) -> String {
//     match env {
//         Environment::Dev => firebase::DEV_API_SECRET.to_string(),
//         Environment::Staging => firebase::STAGING_API_SECRET.to_string(),
//         Environment::Beta => firebase::BETA_API_SECRET.to_string(),
//         _ => String::new(),
//     }
// }
