#[cfg(debug_assertions)]
pub fn analytics_token(_ws_addr_string: &str) -> String {
    // Use one of the tokens in the release version if debugging analytics
    "".into()
}

#[cfg(not(debug_assertions))]
pub fn analytics_token(ws_addr_string: &str) -> String {
    if ws_addr_string.starts_with("wss://dev.orchestrator.nexus.xyz:443/") {
        return "".into(); // TODO: Firebase Analytics tid
    } else if ws_addr_string.starts_with("wss://staging.orchestrator.nexus.xyz:443/") {
        return "".into(); // TODO: Firebase Analytics tid
    } else if ws_addr_string.starts_with("wss://beta.orchestrator.nexus.xyz:443/") {
        return "".into(); // TODO: Firebase Analytics tid
    } else {
        return "".into();
    };
}
