//! Value4Value settings, preview, boost, and the quiet per-minute stream tick.
//!
//! The wallet string is sent only when the user types a new one. The server
//! never returns it.

use gloo_net::http::Request;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ValueSettings {
    pub enabled: bool,
    #[serde(default)]
    pub has_secret: bool,
    #[serde(default)]
    pub max_boost_sats: i64,
    #[serde(default)]
    pub stream_sats_per_minute: i64,
    #[serde(default)]
    pub daily_limit_sats: i64,
    #[serde(default)]
    pub spent_today_sats: i64,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ValueTest {
    pub ok: bool,
    #[serde(default)]
    pub balance_sats: u64,
    #[serde(default)]
    pub detail: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ValuePreview {
    pub payable: bool,
    #[serde(default)]
    pub suggested_sats: Option<i64>,
    #[serde(default)]
    pub recipient_count: i64,
    #[serde(default)]
    pub remote_feed_not_loaded: bool,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ValueParty {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub sats: i64,
    #[serde(default)]
    pub detail: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ValueSend {
    pub sent: bool,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub sats_sent: i64,
    #[serde(default)]
    pub paid: Vec<ValueParty>,
    #[serde(default)]
    pub failed: Vec<ValueParty>,
    #[serde(default)]
    pub unallocated_sats: i64,
}

pub fn api_error_message(status: u16, body: &str) -> String {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("message")
                .and_then(|m| m.as_str())
                .map(|s| s.to_string())
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("request failed ({status})"))
}

async fn send(method: &str, url: &str, api_key: &str, body: Option<Value>) -> Result<String, String> {
    let builder = match method {
        "PUT" => Request::put(url),
        "POST" => Request::post(url),
        _ => Request::get(url),
    }
    .header("Api-Key", api_key);
    let request = if let Some(body) = body {
        builder
            .header("Content-Type", "application/json")
            .body(body.to_string())
            .map_err(|err| err.to_string())?
    } else {
        builder.build().map_err(|err| err.to_string())?
    };
    let response = request.send().await.map_err(|err| err.to_string())?;
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    if !(200..300).contains(&status) {
        return Err(api_error_message(status, &text));
    }
    Ok(text)
}

pub async fn get_settings(server: &str, api_key: &str, user_id: i32) -> Result<ValueSettings, String> {
    let url = format!("{server}/api/data/value/settings?user_id={user_id}");
    let text = send("GET", &url, api_key, None).await?;
    serde_json::from_str(&text).map_err(|err| err.to_string())
}

pub async fn save_settings(
    server: &str,
    api_key: &str,
    user_id: i32,
    enabled: bool,
    secret: &str,
    clear_secret: bool,
    max_boost_sats: i64,
    stream_sats_per_minute: i64,
    daily_limit_sats: i64,
) -> Result<ValueSettings, String> {
    let url = format!("{server}/api/data/value/settings");
    let secret = if secret.trim().is_empty() {
        Value::Null
    } else {
        Value::String(secret.trim().to_string())
    };
    let body = json!({
        "user_id": user_id,
        "enabled": enabled,
        "secret": secret,
        "clear_secret": clear_secret,
        "max_boost_sats": max_boost_sats,
        "stream_sats_per_minute": stream_sats_per_minute,
        "daily_limit_sats": daily_limit_sats,
    });
    let text = send("PUT", &url, api_key, Some(body)).await?;
    serde_json::from_str(&text).map_err(|err| err.to_string())
}

pub async fn test_wallet(server: &str, api_key: &str, user_id: i32, secret: &str) -> Result<ValueTest, String> {
    let url = format!("{server}/api/data/value/test");
    let secret = if secret.trim().is_empty() {
        Value::Null
    } else {
        Value::String(secret.trim().to_string())
    };
    let text = send("POST", &url, api_key, Some(json!({ "user_id": user_id, "secret": secret }))).await?;
    serde_json::from_str(&text).map_err(|err| err.to_string())
}

pub async fn preview_episode(server: &str, api_key: &str, user_id: i32, episode_id: i32) -> Result<ValuePreview, String> {
    let url = format!("{server}/api/data/value/episode?user_id={user_id}&episode_id={episode_id}");
    let text = send("GET", &url, api_key, None).await?;
    serde_json::from_str(&text).map_err(|err| err.to_string())
}

pub async fn send_boost(
    server: &str,
    api_key: &str,
    user_id: i32,
    episode_id: i32,
    sats: i64,
    message: &str,
    sender_name: &str,
    position_seconds: i64,
) -> Result<ValueSend, String> {
    let url = format!("{server}/api/data/value/boost");
    let body = json!({
        "user_id": user_id,
        "episode_id": episode_id,
        "sats": sats,
        "message": message,
        "sender_name": sender_name,
        "position_seconds": position_seconds,
    });
    let text = send("POST", &url, api_key, Some(body)).await?;
    serde_json::from_str(&text).map_err(|err| err.to_string())
}

/// One streaming minute. A failure here must not interrupt playback.
pub async fn call_value_stream(
    server: &str,
    api_key: &str,
    user_id: i32,
    episode_id: i32,
    position_seconds: i64,
) -> Result<ValueSend, String> {
    let url = format!("{server}/api/data/value/stream");
    let body = json!({
        "user_id": user_id,
        "episode_id": episode_id,
        "position_seconds": position_seconds,
    });
    let text = send("POST", &url, api_key, Some(body)).await?;
    serde_json::from_str(&text).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_body_uses_the_server_message() {
        let body = r#"{"error":"Bad request","message":"Bad request: set a daily limit above 0 before sending sats","status_code":400}"#;
        assert!(api_error_message(400, body).contains("daily limit"));
        assert_eq!(api_error_message(500, "nope"), "request failed (500)");
    }
}
