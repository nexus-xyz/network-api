use crate::config::{analytics_api_key, analytics_id};
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
    print_description: bool,
) {
    if print_description {
        println!("{}", description);
    }

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

    // Firebase format for properties for Measurement protocol:
    // https://developers.google.com/analytics/devguides/collection/protocol/ga4/reference?client_type=firebase#payload
    // https://developers.google.com/analytics/devguides/collection/protocol/ga4/reference?client_type=firebase#payload_query_parameters

    let system_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|e| {
            eprintln!("Error calculating system time: {}", e);
            std::time::Duration::from_secs(0) // fallback to epoch start
        })
        .as_millis();

    let timezone = iana_time_zone::get_timezone().ok().map_or_else(
        || String::from("UTC"), // fallback to UTC
        |tz| tz,
    );

    let mut properties = json!({
        "time": system_time,
        // app_instance_id is the standard key Firebase uses this key to track the same user across sessions
        // It is a bit redundant, but I wanted to keep the recommended format Firebase uses to minimize surprises
        // I still left the distinct_id key as well for backwards compatibility
        "app_instance_id": event_properties["prover_id"],
        "distinct_id": event_properties["prover_id"],
        "prover_type": "volunteer",
        "client_type": "cli",
        "operating_system": env::consts::OS,
        "time_zone": timezone,
        "local_hour": local_now.hour(),
        "local_weekday_number_from_monday": local_now.weekday().number_from_monday(),
        "ws_addr_string": ws_addr_string,
    });

    // Add event properties to the properties JSON
    // This is done by iterating over the key-value pairs in the event_properties JSON object
    // but checking that it is a valid JSON object first
    match event_properties.as_object() {
        Some(obj) => {
            for (k, v) in obj {
                properties[k] = v.clone();
            }
        }
        None => eprintln!("Warning: event_properties is not a valid JSON object"),
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

        let url = format!(
            "https://www.google-analytics.com/mp/collect?firebase_app_id={}&api_secret={}",
            firebase_app_id, firebase_api_key
        );

        match client
            .post(&url)
            .body(format!("[{}]", body))
            .header(ACCEPT, "text/plain")
            .header(CONTENT_TYPE, "application/json")
            .send()
            .await
        {
            Ok(response) => match response.text().await {
                Ok(_) => {}
                Err(e) => {
                    eprintln!(
                        "Failed to read analytics response for event '{}': {}",
                        event_name, e
                    );
                }
            },
            Err(e) => {
                eprintln!(
                    "Failed to send analytics request for event '{}' to {}: {}",
                    event_name, url, e
                );
            }
        }
    });
}
