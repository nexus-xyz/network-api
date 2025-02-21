use crate::config::{analytics_api_key, analytics_id, Environment};
use chrono::Datelike;
use chrono::Timelike;
use reqwest::header::ACCEPT;
use serde_json::{json, Value};
use std::{
    env,
    time::{SystemTime, UNIX_EPOCH},
};

pub fn track(
    event_name: String,
    description: String,
    event_properties: Value,
    print_description: bool,
    environment: &Environment,
    client_id: String,
) {
    if print_description {
        println!("{}", description);
    }

    let analytics_id = analytics_id(environment);
    let analytics_api_key = analytics_api_key(environment);

    if analytics_id.is_empty() {
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
        "platform": "CLI",
        "os": env::consts::OS,
        "os_version": env::consts::OS,  // We could get more specific version if needed
        "app_version": env!("CARGO_PKG_VERSION"),
        "node_id": event_properties["node_id"],
        "timezone": timezone,
        "local_hour": local_now.hour(),
        "day_of_week": local_now.weekday().number_from_monday(),
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

    // Format for events
    let body = json!({
        "client_id": client_id,
        "events": [{
            "name": event_name,
            "params": properties
        }],
    });

    tokio::spawn(async move {
        let client = reqwest::Client::new();

        let url = format!(
            "https://www.google-analytics.com/mp/collect?measurement_id={}&api_secret={}",
            analytics_id, analytics_api_key
        );

        match client
            .post(&url)
            .json(&body)
            .header(ACCEPT, "application/json")
            .send()
            .await
        {
            Ok(response) => {
                let status = response.status();
                if !status.is_success() {
                    match response.text().await {
                        Ok(error_text) => {
                            eprintln!(
                                "Analytics request failed for event '{}' with status {}: {}",
                                event_name, status, error_text
                            );
                        }
                        Err(e) => {
                            eprintln!(
                                "Analytics request failed for event '{}' with status {}. Failed to read error response: {}",
                                event_name, status, e
                            );
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!(
                    "Failed to send analytics request for event '{}' to {}: {}",
                    event_name, url, e
                );
            }
        }
    });
}
