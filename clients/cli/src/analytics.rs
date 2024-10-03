use crate::config::analytics_token;
use chrono::Datelike;
use chrono::Timelike;
use reqwest::header::{ACCEPT, CONTENT_TYPE};
use serde_json::{json, Value};
use std::{
    env,
    time::{SystemTime, UNIX_EPOCH},
};

pub fn track(
    event_name: String,
    description: String,
    ws_addr_string: &str,
    event_properties: Value,
) {
    println!("{}", description);

    let token = analytics_token(ws_addr_string);
    if token.is_empty() {
        return;
    }
    let local_now = chrono::offset::Local::now();
    let mut properties = json!({
        "token": token,
        "time": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis(),
        "distinct_id": event_properties["prover_id"],
        "prover_type": "volunteer",
        "client_type": "cli",
        "operating_system": env::consts::OS,
        "time_zone": iana_time_zone::get_timezone().unwrap(),
        "local_hour": local_now.hour(),
        "local_weekday_number_from_monday": local_now.weekday().number_from_monday(),
        "ws_addr_string": ws_addr_string,
    });
    for (k, v) in event_properties.as_object().unwrap() {
        properties[k] = v.clone();
    }
    let body = json!({
        "event": event_name,
        "properties": properties
    });
    tokio::spawn(async move {
        let client = reqwest::Client::new();
        let _ = client
            .post("https://api.mixpanel.com/track?ip=1")
            .body(format!("[{}]", body.to_string()))
            .header(ACCEPT, "text/plain")
            .header(CONTENT_TYPE, "application/json")
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
    });
}
