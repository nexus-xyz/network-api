// Debug version of analytics_id
#[cfg(debug_assertions)]
pub fn analytics_id(_ws_addr_string: &str) -> String {
    // Use one of the tokens in the release version if debugging analytics
    "".into()
}

// Debug version of analytics_api_key
#[cfg(debug_assertions)]
pub fn analytics_api_key(_ws_addr_string: &str) -> String {
    // Use one of the tokens in the release version if debugging analytics
    "".into()
}

// The following enum is used to determine the environment from the web socket string
#[derive(Debug)]
#[cfg(not(debug_assertions))]
enum Environment {
    Dev,
    Staging,
    Beta,
    Unknown,
}

// The web socket addresses for the different environments
#[cfg(not(debug_assertions))]
mod web_socket_urls {
    pub const DEV: &str = "wss://dev.orchestrator.nexus.xyz:443/";
    pub const STAGING: &str = "wss://staging.orchestrator.nexus.xyz:443/";
    pub const BETA: &str = "wss://beta.orchestrator.nexus.xyz:443/";
}

// the firebase APP IDS by environment
#[cfg(not(debug_assertions))]
mod firebase {
    pub const DEV_APP_ID: &str = "1:954530464230:web:f0a14de14ef7bcdaa99627";
    pub const STAGING_APP_ID: &str = "1:222794630996:web:1758d64a85eba687eaaac1";
    pub const BETA_APP_ID: &str = "1:279395003658:web:04ee2c524474d683d75ef3";

    // Analytics keys for the different environments
    // These are keys that allow the measurement protocol to write to the analytics database
    // They are not sensitive. Worst case, if a malicious actor obtains the secret, they could potentially send false or misleading data to your GA4 property
    pub const DEV_API_SECRET: &str = "8ySxiKrtT8a76zClqqO8IQ";
    pub const STAGING_API_SECRET: &str = "OI7H53soRMSDWfJf1ittHQ";
    pub const BETA_API_SECRET: &str = "gxxzKAQLSl-uYI0eKbIi_Q";
}

// Release versions (existing code)
#[cfg(not(debug_assertions))]
pub fn analytics_id(ws_addr_string: &str) -> String {
    // Determine the environment from the web socket string (ws_addr_string)
    let env = match ws_addr_string {
        web_socket_urls::DEV => Environment::Dev,
        web_socket_urls::STAGING => Environment::Staging,
        web_socket_urls::BETA => Environment::Beta,
        _ => Environment::Unknown,
    };

    // Return the appropriate Firebase App ID based on the environment
    match env {
        Environment::Dev => firebase::DEV_APP_ID.to_string(),
        Environment::Staging => firebase::STAGING_APP_ID.to_string(),
        Environment::Beta => firebase::BETA_APP_ID.to_string(),
        Environment::Unknown => String::new(),
    }
}

#[cfg(not(debug_assertions))]
pub fn analytics_api_key(ws_addr_string: &str) -> String {
    match ws_addr_string {
        web_socket_urls::DEV => firebase::DEV_API_SECRET.to_string(),
        web_socket_urls::STAGING => firebase::STAGING_API_SECRET.to_string(),
        web_socket_urls::BETA => firebase::BETA_API_SECRET.to_string(),
        _ => String::new(),
    }
}
