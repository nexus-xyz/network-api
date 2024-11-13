use crate::config::{analytics_id, analytics_api_key};
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

    let firebase_app_id = analytics_id(ws_addr_string);
    let firebase_api_key = analytics_api_key(ws_addr_string);
    if firebase_app_id.is_empty() {
        return;
    }
    let local_now = chrono::offset::Local::now();

    // For tracking events, we use the Firebase Measurement Protocol
    // Firebase is mostly designed for mobile and web apps, but for our use case of a CLI,
    // we can use the Measurement Protocol to track events by POST to a URL. 
    // The only thing that may be unexpected is that the URL we use includes a firebase key

    // Firebase format for properties for Mesurement protocol: 
    // https://developers.google.com/analytics/devguides/collection/protocol/ga4/reference?client_type=firebase#payload
    // https://developers.google.com/analytics/devguides/collection/protocol/ga4/reference?client_type=firebase#payload_query_parameters
    let mut properties = json!({
        "time": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis(),
        // app_instance_id is the standard key Firebase uses this key to track the same user across sessions
        // its is a bit redundant, but I wanted to keep the recommended format Firebase uses to minimize surprises
        // I still left the distinct_id key as well for backwards compatibility
        "app_instance_id": event_properties["prover_id"],  
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

    // Firebase format for events
    let body = json!({
        "app_instance_id": event_properties["prover_id"],
        "events": [{
            "name": event_name,
            "params": properties
        }],
    });

    tokio::spawn(async move {
        let client = reqwest::Client::new();

        let _ = client
            //URL is the google analytics endpoint for firebase: https://stackoverflow.com/questions/50355752/firebase-analytics-from-remote-rest-api
            .post(format!(
                "https://www.google-analytics.com/mp/collect?firebase_app_id={}&api_secret={}",
                firebase_app_id,
                firebase_api_key
            ))
            .body(format!("[{}]", body))
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
