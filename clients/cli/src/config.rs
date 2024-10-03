pub fn analytics_token(ws_addr_string: &str) -> String {
    if ws_addr_string.starts_with("wss://dev.orchestrator.nexus.xyz:443/") {
        return "504d4d443854f2cd10e2e385aca81aa4".into();
    } else if ws_addr_string.starts_with("wss://staging.orchestrator.nexus.xyz:443/") {
        return "30bcb58893992aabc5aec014e7b903d2".into();
    } else if ws_addr_string.starts_with("wss://beta.orchestrator.nexus.xyz:443/") {
        return "3c16d3853f4258414c9c9109bbbdef0e".into();
    } else {
        return "".into();
    };
}
