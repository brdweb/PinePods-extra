//! Music Assistant as a PinePods audio output.
//!
//! A per-user, opt-in bridge dials Music Assistant's WebSocket
//! (`ws://host:8095/ws`, first command `auth`), registers each available player
//! or group as a now-playing virtual device, and turns relay commands into MA
//! `players/cmd/*` and `player_queues/*` calls. A downloaded file is served from
//! the PinePods stream URL (podcast-scoped RSS key). An episode that was never
//! downloaded is the remote enclosure, not that stream URL, which 404s until a
//! file exists. Queue events write listen progress back.
//!
//! Nothing here runs unless the user saved an enabled configuration. The
//! WebSocket client is the only extra crate, and it is unused until then.

use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::ws::Message as RelayMessage;
use futures::{Sink, SinkExt, Stream, StreamExt};
use serde_json::{json, Value};
use sqlx::Row;
use tokio::sync::{mpsc, oneshot, watch, Notify, RwLock};
use tokio_tungstenite::tungstenite::Message;

use crate::database::DatabasePool;
use crate::error::{AppError, AppResult};
use crate::handlers::nowplaying::notify_devices_changed;
use crate::models::NowPlayingSnapshot;
use crate::AppState;

/// Actions a Music Assistant virtual device announces on the now-playing relay.
pub const MA_CAPABILITIES: &[&str] = &[
    "play",
    "pause",
    "stop",
    "seek",
    "skip_forward",
    "skip_back",
    "set_speed",
    "play_episode",
    "volume_set",
    "volume_mute",
    "group",
    "group_many",
    "ungroup",
    "set_members",
    "group_volume",
];

const DEVICE_TYPE: &str = "music_assistant";
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(15);
const COMMAND_TIMEOUT: Duration = Duration::from_secs(20);
const PLAYER_REFRESH: Duration = Duration::from_secs(20);
const PROGRESS_WRITE_EVERY: Duration = Duration::from_secs(10);

// ============================ URLs and versions ============================

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaEndpoints {
    /// `http://host:port` (or https), stored and shown in settings.
    pub origin: String,
    pub ws: String,
    pub api: String,
}

/// Normalize a Music Assistant base URL to its WebSocket and JSON-RPC endpoints.
///
/// Accepts `http`, `https`, `ws`, and `wss`. Loopback is allowed (PinePods and
/// Music Assistant on the same machine). Link-local and cloud-metadata addresses
/// are rejected. A missing port becomes 8095 for plain http, 443 for https.
pub fn parse_server_url(raw: &str) -> Result<MaEndpoints, String> {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("Music Assistant URL is empty".into());
    }
    let (secure, rest) = if let Some(rest) = trimmed.strip_prefix("wss://") {
        (true, format!("https://{rest}"))
    } else if let Some(rest) = trimmed.strip_prefix("ws://") {
        (false, format!("http://{rest}"))
    } else if trimmed.starts_with("https://") {
        (true, trimmed.to_string())
    } else if trimmed.starts_with("http://") {
        (false, trimmed.to_string())
    } else {
        return Err("Music Assistant URL must be http, https, ws, or wss".into());
    };
    let url = url::Url::parse(&rest).map_err(|e| format!("invalid Music Assistant URL: {e}"))?;
    if !url.username().is_empty() || url.password().is_some() {
        return Err("Music Assistant URL must not include credentials".into());
    }
    let path = url.path().trim_end_matches('/');
    if !path.is_empty() && path != "/ws" && path != "/api" {
        return Err(
            "Music Assistant URL should be the server origin, for example http://192.168.1.20:8095"
                .into(),
        );
    }
    let host = url
        .host_str()
        .ok_or_else(|| "Music Assistant URL has no host".to_string())?;
    if is_link_local_or_metadata(host) {
        return Err(
            "Music Assistant URL must not point at a link-local or metadata address".into(),
        );
    }
    let port = url.port().unwrap_or(if secure { 443 } else { 8095 });
    let hostport = format!("{host}:{port}");
    let http_scheme = if secure { "https" } else { "http" };
    let ws_scheme = if secure { "wss" } else { "ws" };
    Ok(MaEndpoints {
        origin: format!("{http_scheme}://{hostport}"),
        ws: format!("{ws_scheme}://{hostport}/ws"),
        api: format!("{http_scheme}://{hostport}/api"),
    })
}

/// Origin Music Assistant will use to fetch `/api/data/stream/...`.
///
/// Must be http(s) with no path. Loopback is rejected: MA is another machine
/// (or another container) and cannot use PinePods' localhost. Private LAN
/// addresses are allowed.
pub fn normalize_external_base(raw: &str) -> Result<String, String> {
    let url = url::Url::parse(raw.trim()).map_err(|e| format!("invalid external base URL: {e}"))?;
    if url.scheme() != "http" && url.scheme() != "https" {
        return Err("external base URL must be http or https".into());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("external base URL must not include credentials".into());
    }
    if url.path() != "/" && !url.path().is_empty() {
        return Err(
            "external base URL must be an origin without a path (for example http://192.168.1.20:8040)"
                .into(),
        );
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err("external base URL must not include a query or fragment".into());
    }
    let host = url
        .host_str()
        .ok_or_else(|| "external base URL has no host".to_string())?;
    if is_unreachable_pinepods_host(host) {
        return Err(
            "external base URL must be a LAN or public host Music Assistant can reach, not localhost"
                .into(),
        );
    }
    let origin = match url.port() {
        Some(p) => format!("{}://{host}:{p}", url.scheme()),
        None => format!("{}://{host}", url.scheme()),
    };
    Ok(origin)
}

fn host_bare(host: &str) -> &str {
    host.trim_matches(|c| c == '[' || c == ']')
}

fn is_link_local_or_metadata(host: &str) -> bool {
    let bare = host_bare(host);
    let lower = bare.to_ascii_lowercase();
    if lower == "metadata.google.internal" || lower.ends_with(".metadata.google.internal") {
        return true;
    }
    match bare.parse::<IpAddr>() {
        Ok(IpAddr::V4(v4)) => v4.is_link_local(),
        Ok(IpAddr::V6(v6)) => (v6.segments()[0] & 0xffc0) == 0xfe80,
        Err(_) => false,
    }
}

fn is_unreachable_pinepods_host(host: &str) -> bool {
    let bare = host_bare(host);
    let lower = bare.to_ascii_lowercase();
    if lower == "localhost" || lower == "localhost.localdomain" || lower.ends_with(".localhost") {
        return true;
    }
    if is_link_local_or_metadata(host) {
        return true;
    }
    match bare.parse::<IpAddr>() {
        Ok(IpAddr::V4(v4)) => v4.is_loopback() || v4.is_unspecified(),
        Ok(IpAddr::V6(v6)) => v6.is_loopback() || v6.is_unspecified(),
        Err(_) => false,
    }
}

/// `2.10.1`, `v2.10.0-beta` → `(2, 10, 1)`.
pub fn parse_version(raw: &str) -> Option<(u64, u64, u64)> {
    let s = raw.trim().trim_start_matches(['v', 'V']);
    let s = s.split(['-', '+', ' ']).next().unwrap_or(s);
    let mut parts = s.split('.');
    let major: u64 = parts.next()?.parse().ok()?;
    let minor: u64 = parts.next().unwrap_or("0").parse().ok()?;
    let patch_src = parts.next().unwrap_or("0");
    let patch_digits: String = patch_src
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    let patch = if patch_digits.is_empty() {
        0
    } else {
        patch_digits.parse().ok()?
    };
    Some((major, minor, patch))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionAssessment {
    pub compatible: bool,
    /// Set when we connected but the server is outside the tested 2.10 line.
    pub warning: Option<String>,
}

/// 2.10.x is the tested line. Older than 2.6 is refused. Newer majors and
/// missing version fields warn and continue, so a schema bump does not brick playback.
pub fn assess_version(server_version: &str, schema_version: Option<i64>) -> VersionAssessment {
    let version = server_version.trim();
    if version.is_empty() {
        return VersionAssessment {
            compatible: true,
            warning: Some("Music Assistant did not report a version; continuing".into()),
        };
    }
    let Some((major, minor, _)) = parse_version(version) else {
        return VersionAssessment {
            compatible: true,
            warning: Some(format!(
                "could not parse Music Assistant version '{version}'; continuing"
            )),
        };
    };
    if major < 2 || (major == 2 && minor < 6) {
        return VersionAssessment {
            compatible: false,
            warning: Some(format!(
                "Music Assistant {version} is too old; 2.10.x is required"
            )),
        };
    }
    let mut warning = None;
    if major >= 3 {
        warning = Some(format!(
            "Music Assistant {version} is newer than the tested 2.10.x line; commands may differ"
        ));
    } else if minor < 10 {
        warning = Some(format!(
            "Music Assistant {version} is older than the tested 2.10.x line; playback should work, newer commands might not"
        ));
    }
    if let Some(schema) = schema_version {
        if schema < 20 {
            let extra = format!("schema_version {schema} is older than expected for 2.10");
            warning = Some(match warning {
                Some(w) => format!("{w}; {extra}"),
                None => extra,
            });
        }
    }
    VersionAssessment {
        compatible: true,
        warning,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerInfo {
    pub server_version: String,
    pub schema_version: Option<i64>,
    pub server_id: Option<String>,
    pub assessment: VersionAssessment,
}

pub fn server_info_from_result(v: &Value) -> ServerInfo {
    let server_version = v
        .get("server_version")
        .and_then(|s| s.as_str())
        .unwrap_or("")
        .to_string();
    let schema_version = v.get("schema_version").and_then(json_i64);
    let server_id = v
        .get("server_id")
        .and_then(|s| s.as_str())
        .map(|s| s.to_string());
    let assessment = assess_version(&server_version, schema_version);
    ServerInfo {
        server_version,
        schema_version,
        server_id,
        assessment,
    }
}

/// Where Music Assistant should load an episode from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaPlayback {
    /// PinePods can serve a file (a download, or `local://` media).
    Stream,
    /// Remote http(s) enclosure. Never a PinePods stream URL.
    Direct(String),
}

fn is_http_url(uri: &str) -> bool {
    let lower = uri.trim().to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// `served_by_pinepods` is a file the stream endpoint can actually return.
/// Anything else must be a playable http(s) enclosure. The stream URL is not a
/// fallback: it 404s when the episode was never saved.
pub fn ma_playback(
    served_by_pinepods: bool,
    direct_uri: Option<String>,
) -> Result<MaPlayback, &'static str> {
    if served_by_pinepods {
        return Ok(MaPlayback::Stream);
    }
    match direct_uri.filter(|uri| is_http_url(uri)) {
        Some(uri) => Ok(MaPlayback::Direct(uri)),
        None => Err(
            "This episode has no URL Music Assistant can play. Download it in PinePods first.",
        ),
    }
}

pub fn build_stream_url(
    external_base: &str,
    episode_id: i32,
    rss_key: &str,
    user_id: i32,
    is_youtube: bool,
) -> String {
    let base = external_base.trim_end_matches('/');
    let key = urlencoding::encode(rss_key);
    let mut url = format!("{base}/api/data/stream/{episode_id}?api_key={key}&user_id={user_id}");
    if is_youtube {
        url.push_str("&type=youtube");
    }
    url
}

/// `ma:{user_id}:{player_id}`. Player ids may themselves contain colons.
pub fn device_id_for(user_id: i32, player_id: &str) -> String {
    format!("ma:{user_id}:{player_id}")
}

pub fn player_id_from_device(user_id: i32, device_id: &str) -> Option<String> {
    let prefix = format!("ma:{user_id}:");
    let rest = device_id.strip_prefix(&prefix)?;
    if rest.is_empty() {
        None
    } else {
        Some(rest.to_string())
    }
}

pub fn canonical_player_id(user_id: i32, raw: &str) -> String {
    player_id_from_device(user_id, raw).unwrap_or_else(|| raw.to_string())
}

// ============================ Players and progress ============================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaPlayerKind {
    Player,
    Group,
}

impl MaPlayerKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Player => "player",
            Self::Group => "group",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MaPlayer {
    pub player_id: String,
    pub name: String,
    pub available: bool,
    pub state: String,
    pub volume_level: Option<f64>,
    pub volume_muted: bool,
    pub kind: MaPlayerKind,
    pub group_members: Vec<String>,
    pub synced_to: Option<String>,
    pub can_group_with: Vec<String>,
    pub active_source: Option<String>,
    pub elapsed_time: Option<f64>,
    pub current_uri: Option<String>,
    pub current_title: Option<String>,
}

pub fn players_from_result(v: &Value) -> Vec<MaPlayer> {
    let list = if let Some(arr) = v.as_array() {
        arr
    } else if let Some(arr) = v.get("players").and_then(|p| p.as_array()) {
        arr
    } else {
        return Vec::new();
    };
    list.iter().filter_map(player_from_value).collect()
}

pub fn player_from_value(v: &Value) -> Option<MaPlayer> {
    let player_id = v.get("player_id").and_then(|s| s.as_str())?.to_string();
    if player_id.is_empty() {
        return None;
    }
    let name = v
        .get("name")
        .or_else(|| v.get("display_name"))
        .and_then(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(&player_id)
        .to_string();
    let group_members = id_list(v.get("group_childs").unwrap_or(&Value::Null));
    let type_name = v.get("type").and_then(|s| s.as_str()).unwrap_or("");
    let kind = if type_name.to_ascii_lowercase().contains("group") {
        MaPlayerKind::Group
    } else {
        MaPlayerKind::Player
    };
    let state = v
        .get("state")
        .and_then(|s| s.as_str())
        .unwrap_or("idle")
        .to_string();
    let synced_to = v
        .get("synced_to")
        .and_then(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let active_source = v
        .get("active_source")
        .and_then(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    Some(MaPlayer {
        player_id,
        name,
        available: v.get("available").and_then(|b| b.as_bool()).unwrap_or(true),
        state,
        volume_level: v.get("volume_level").and_then(json_f64),
        volume_muted: v
            .get("volume_muted")
            .and_then(|b| b.as_bool())
            .unwrap_or(false),
        kind,
        group_members,
        synced_to,
        can_group_with: id_list(v.get("can_group_with").unwrap_or(&Value::Null)),
        active_source,
        elapsed_time: v.get("elapsed_time").and_then(json_f64),
        current_uri: first_uri(v),
        current_title: first_title(v),
    })
}

fn id_list(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    item.as_str().map(|s| s.to_string()).or_else(|| {
                        item.get("player_id")
                            .and_then(|p| p.as_str())
                            .map(|s| s.to_string())
                    })
                })
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn first_uri(v: &Value) -> Option<String> {
    for key in ["current_media", "current_item"] {
        if let Some(node) = v.get(key) {
            if let Some(uri) = node.get("uri").and_then(|s| s.as_str()) {
                return Some(uri.to_string());
            }
            if let Some(uri) = node
                .get("media_item")
                .and_then(|m| m.get("uri"))
                .and_then(|s| s.as_str())
            {
                return Some(uri.to_string());
            }
        }
    }
    None
}

fn first_title(v: &Value) -> Option<String> {
    for key in ["current_media", "current_item"] {
        if let Some(node) = v.get(key) {
            if let Some(name) = node
                .get("name")
                .or_else(|| node.get("title"))
                .and_then(|s| s.as_str())
            {
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
        }
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamRef {
    pub episode_id: i32,
    pub user_id: Option<i32>,
    pub is_youtube: bool,
}

/// Pull a PinePods stream URL (`/api/data/stream/{id}?api_key=…&user_id=…`) out of any string.
pub fn parse_stream_url(raw: &str) -> Option<StreamRef> {
    let marker = "/api/data/stream/";
    let idx = raw.find(marker)?;
    let rest = &raw[idx + marker.len()..];
    let id_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if id_str.is_empty() {
        return None;
    }
    let episode_id: i32 = id_str.parse().ok()?;
    let query = raw.split_once('?').map(|(_, q)| q).unwrap_or("");
    let mut user_id = None;
    let mut is_youtube = false;
    for pair in query.split(['&', '#']) {
        let mut it = pair.splitn(2, '=');
        let key = it.next().unwrap_or("").trim_start_matches("amp;");
        let val = it.next().unwrap_or("");
        if key == "user_id" {
            user_id = val.parse().ok();
        } else if key == "type" && val == "youtube" {
            is_youtube = true;
        }
    }
    Some(StreamRef {
        episode_id,
        user_id,
        is_youtube,
    })
}

pub fn find_stream_ref(v: &Value) -> Option<StreamRef> {
    match v {
        Value::String(s) => parse_stream_url(s),
        Value::Array(items) => items.iter().find_map(find_stream_ref),
        Value::Object(map) => map.values().find_map(find_stream_ref),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProgressUpdate {
    pub queue_id: Option<String>,
    pub stream: Option<StreamRef>,
    pub position_sec: Option<f64>,
    pub duration_sec: Option<f64>,
    pub playing: Option<bool>,
    pub title: Option<String>,
    pub speed: Option<f64>,
}

/// Read whatever a queue or player event tells us about PinePods playback.
pub fn progress_from_payload(data: &Value) -> ProgressUpdate {
    let queue_id = data
        .get("queue_id")
        .or_else(|| data.get("player_id"))
        .and_then(|s| s.as_str())
        .map(|s| s.to_string());
    let position_sec = data
        .get("elapsed_time")
        .and_then(json_f64)
        .or_else(|| data.get("elapsed_time").and_then(json_f64));
    let state = data.get("state").and_then(|s| s.as_str());
    let playing = state.map(|s| s == "playing" || s == "buffering");
    let item = data
        .get("current_item")
        .or_else(|| data.get("current_media"));
    let duration_sec = item
        .and_then(|i| i.get("duration"))
        .and_then(json_f64)
        .or_else(|| {
            item.and_then(|i| i.get("media_item"))
                .and_then(|m| m.get("duration"))
                .and_then(json_f64)
        });
    let title = item.and_then(|i| {
        i.get("name")
            .or_else(|| i.get("title"))
            .or_else(|| i.get("media_item").and_then(|m| m.get("name")))
            .and_then(|s| s.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    });
    let speed = data.get("playback_speed").and_then(json_f64).or_else(|| {
        item.and_then(|i| i.get("playback_speed"))
            .and_then(json_f64)
    });
    ProgressUpdate {
        queue_id,
        stream: find_stream_ref(data),
        position_sec,
        duration_sec,
        playing,
        title,
        speed,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProgressSample {
    pub episode_id: i32,
    pub position_sec: f64,
    pub playing: bool,
}

/// Throttle history writes: every 10s, on pause, on episode change, or on a seek-sized jump.
pub fn should_write_progress(
    prev: Option<&ProgressSample>,
    next: &ProgressSample,
    since_write: Duration,
) -> bool {
    let Some(prev) = prev else {
        return true;
    };
    if prev.episode_id != next.episode_id {
        return true;
    }
    if prev.playing && !next.playing {
        return true;
    }
    if (next.position_sec - prev.position_sec).abs() >= 8.0 {
        return true;
    }
    since_write >= PROGRESS_WRITE_EVERY
}

pub fn queue_id_for(players: &[MaPlayer], player_id: &str) -> String {
    players
        .iter()
        .find(|p| p.player_id == player_id)
        .and_then(|p| p.active_source.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| player_id.to_string())
}

// ============================ Commands ============================

#[derive(Debug, Clone, PartialEq)]
pub struct MaCommand {
    pub command: String,
    pub args: Value,
}

/// Map a relay/REST action onto one or more Music Assistant commands.
/// `play_episode` is expanded by [`prepare_play_episode`] because it needs the database.
pub fn translate_action(
    player_id: &str,
    queue_id: &str,
    action: &str,
    args: &Value,
) -> Result<Vec<MaCommand>, String> {
    let player_cmd = |name: &str, extra: Value| MaCommand {
        command: format!("players/cmd/{name}"),
        args: {
            let mut base = json!({ "player_id": player_id });
            if let (Some(obj), Some(extra_obj)) = (base.as_object_mut(), extra.as_object()) {
                for (k, v) in extra_obj {
                    obj.insert(k.clone(), v.clone());
                }
            }
            base
        },
    };
    let queue_cmd = |name: &str, extra: Value| MaCommand {
        command: format!("player_queues/{name}"),
        args: {
            let mut base = json!({ "queue_id": queue_id });
            if let (Some(obj), Some(extra_obj)) = (base.as_object_mut(), extra.as_object()) {
                for (k, v) in extra_obj {
                    obj.insert(k.clone(), v.clone());
                }
            }
            base
        },
    };

    let one = match action {
        "play" => player_cmd("play", json!({})),
        "pause" => player_cmd("pause", json!({})),
        "stop" => player_cmd("stop", json!({})),
        "seek" => {
            let position = args
                .get("position_sec")
                .or_else(|| args.get("position"))
                .and_then(json_f64)
                .ok_or_else(|| "seek requires position_sec".to_string())?;
            if position < 0.0 {
                return Err("seek position must be >= 0".into());
            }
            queue_cmd("seek", json!({ "position": position }))
        }
        "skip_forward" => {
            let seconds = args.get("seconds").and_then(json_f64).unwrap_or(30.0).abs();
            queue_cmd("skip", json!({ "seconds": seconds }))
        }
        "skip_back" => {
            let seconds = args.get("seconds").and_then(json_f64).unwrap_or(15.0).abs();
            queue_cmd("skip", json!({ "seconds": -seconds }))
        }
        "set_speed" => {
            let speed = args
                .get("speed")
                .and_then(json_f64)
                .ok_or_else(|| "set_speed requires speed".to_string())?;
            if !(0.5..=4.0).contains(&speed) {
                return Err("speed must be between 0.5 and 4".into());
            }
            let mut extra = json!({ "speed": speed });
            if let Some(id) = args.get("queue_item_id").and_then(|s| s.as_str()) {
                extra["queue_item_id"] = json!(id);
            }
            queue_cmd("set_playback_speed", extra)
        }
        "volume_set" => {
            let level = args
                .get("volume_level")
                .or_else(|| args.get("volume"))
                .and_then(json_f64)
                .ok_or_else(|| "volume_set requires volume_level".to_string())?;
            let level = level.clamp(0.0, 100.0);
            player_cmd("volume_set", json!({ "volume_level": level }))
        }
        "volume_mute" => {
            let muted = args
                .get("muted")
                .or_else(|| args.get("mute"))
                .and_then(|v| v.as_bool())
                .ok_or_else(|| "volume_mute requires muted".to_string())?;
            player_cmd("volume_mute", json!({ "muted": muted }))
        }
        "group" => {
            let target = args
                .get("target_player")
                .and_then(|s| s.as_str())
                .filter(|s| !s.is_empty())
                .ok_or_else(|| "group requires target_player".to_string())?;
            player_cmd("group", json!({ "target_player": target }))
        }
        "group_many" => {
            let ids = string_list(args.get("player_ids").or_else(|| args.get("members")));
            if ids.is_empty() {
                return Err("group_many requires player_ids".into());
            }
            let target = args
                .get("target_player")
                .and_then(|s| s.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or(player_id);
            player_cmd(
                "group_many",
                json!({ "target_player": target, "player_ids": ids }),
            )
        }
        "ungroup" => player_cmd("ungroup", json!({})),
        "set_members" => {
            let add = string_list(args.get("player_ids_to_add"));
            let remove = string_list(args.get("player_ids_to_remove"));
            if add.is_empty() && remove.is_empty() {
                return Err(
                    "set_members requires player_ids_to_add or player_ids_to_remove".into(),
                );
            }
            let target = args
                .get("target_player")
                .and_then(|s| s.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or(player_id);
            player_cmd(
                "set_members",
                json!({
                    "target_player": target,
                    "player_ids_to_add": add,
                    "player_ids_to_remove": remove,
                }),
            )
        }
        "group_volume" => {
            let level = args
                .get("volume_level")
                .or_else(|| args.get("volume"))
                .and_then(json_f64)
                .ok_or_else(|| "group_volume requires volume_level".to_string())?;
            player_cmd(
                "group_volume",
                json!({ "volume_level": level.clamp(0.0, 100.0) }),
            )
        }
        other => return Err(format!("unsupported action '{other}'")),
    };
    Ok(vec![one])
}

/// `play_media`, then an optional seek and speed change. Seek/speed failures are warnings.
pub fn play_media_commands(
    queue_id: &str,
    media_url: &str,
    position_sec: f64,
    speed: f64,
) -> Vec<MaCommand> {
    let mut cmds = vec![MaCommand {
        command: "player_queues/play_media".into(),
        args: json!({
            "queue_id": queue_id,
            "media": media_url,
            "option": "replace",
        }),
    }];
    if position_sec >= 1.0 {
        cmds.push(MaCommand {
            command: "player_queues/seek".into(),
            args: json!({ "queue_id": queue_id, "position": position_sec }),
        });
    }
    if speed > 0.0 && (speed - 1.0).abs() > 0.01 {
        cmds.push(MaCommand {
            command: "player_queues/set_playback_speed".into(),
            args: json!({ "queue_id": queue_id, "speed": speed }),
        });
    }
    cmds
}

fn string_list(v: Option<&Value>) -> Vec<String> {
    v.and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|i| i.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn canonicalize_args(user_id: i32, args: &Value) -> Value {
    let mut args = args.clone();
    let Some(obj) = args.as_object_mut() else {
        return args;
    };
    for key in ["target_player"] {
        if let Some(s) = obj.get(key).and_then(|v| v.as_str()) {
            let canon = canonical_player_id(user_id, s);
            obj.insert(key.to_string(), json!(canon));
        }
    }
    for key in [
        "player_ids",
        "members",
        "player_ids_to_add",
        "player_ids_to_remove",
    ] {
        if let Some(arr) = obj.get(key).and_then(|v| v.as_array()) {
            let ids: Vec<String> = arr
                .iter()
                .filter_map(|v| v.as_str())
                .map(|s| canonical_player_id(user_id, s))
                .collect();
            obj.insert(key.to_string(), json!(ids));
        }
    }
    args
}

/// Relayed now-playing frame: `{"type":"command","action","args"}`.
pub fn parse_relay_command(text: &str) -> Option<(String, Value)> {
    let v: Value = serde_json::from_str(text).ok()?;
    if v.get("type").and_then(|t| t.as_str()) != Some("command") {
        return None;
    }
    let action = v.get("action")?.as_str()?.to_string();
    let args = v.get("args").cloned().unwrap_or_else(|| json!({}));
    Some((action, args))
}

// ============================ Wire protocol ============================

#[derive(Debug)]
pub enum Inbound {
    Result {
        message_id: String,
        result: Value,
    },
    Error {
        message_id: String,
        error_code: i64,
        details: String,
    },
    Event {
        event: String,
        object_id: Option<String>,
        data: Value,
    },
    Ignore,
}

pub fn parse_inbound(text: &str) -> Inbound {
    let Ok(v) = serde_json::from_str::<Value>(text) else {
        return Inbound::Ignore;
    };
    if let Some(event) = v.get("event").and_then(|e| e.as_str()) {
        if v.get("message_id").is_none() || v.get("result").is_none() {
            return Inbound::Event {
                event: event.to_string(),
                object_id: v
                    .get("object_id")
                    .and_then(|s| s.as_str())
                    .map(|s| s.to_string()),
                data: v.get("data").cloned().unwrap_or(Value::Null),
            };
        }
    }
    let Some(message_id) = v.get("message_id").and_then(|s| s.as_str()) else {
        return Inbound::Ignore;
    };
    if v.get("error_code").is_some() || (v.get("result").is_none() && v.get("error").is_some()) {
        let details = v
            .get("details")
            .or_else(|| v.get("error"))
            .or_else(|| v.get("message"))
            .and_then(|s| s.as_str())
            .unwrap_or("Music Assistant command failed")
            .to_string();
        return Inbound::Error {
            message_id: message_id.to_string(),
            error_code: v.get("error_code").and_then(json_i64).unwrap_or(0),
            details,
        };
    }
    if let Some(result) = v.get("result") {
        return Inbound::Result {
            message_id: message_id.to_string(),
            result: result.clone(),
        };
    }
    Inbound::Ignore
}

#[derive(Debug)]
enum SessionNote {
    Server(ServerInfo),
    Players(Vec<MaPlayer>),
    Event {
        event: String,
        object_id: Option<String>,
        data: Value,
    },
}

struct Outbound {
    command: String,
    args: Value,
    reply: Option<oneshot::Sender<Result<Value, String>>>,
}

struct Inflight {
    reply: Option<oneshot::Sender<Result<Value, String>>>,
}

/// Speak the Music Assistant WebSocket: auth, `info`, `players/all`, then commands and events.
async fn drive_socket<S>(
    mut socket: S,
    token: &str,
    mut outbound: mpsc::UnboundedReceiver<Outbound>,
    events: mpsc::UnboundedSender<SessionNote>,
) -> Result<(), String>
where
    S: Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>
        + Sink<Message, Error = tokio_tungstenite::tungstenite::Error>
        + Unpin,
{
    let mut next_id: u64 = 1;
    let mut inflight: HashMap<String, Inflight> = HashMap::new();

    let auth_id = send_cmd(
        &mut socket,
        &mut next_id,
        &mut inflight,
        "auth",
        json!({ "token": token }),
        None,
    )
    .await?;
    let auth_result = wait_result(
        &mut socket,
        &mut inflight,
        &events,
        &auth_id,
        HANDSHAKE_TIMEOUT,
    )
    .await?;
    if auth_result.get("authenticated").and_then(|v| v.as_bool()) == Some(false) {
        return Err("Music Assistant rejected the token".into());
    }

    let info_id = send_cmd(
        &mut socket,
        &mut next_id,
        &mut inflight,
        "info",
        json!({}),
        None,
    )
    .await?;
    match wait_result(
        &mut socket,
        &mut inflight,
        &events,
        &info_id,
        HANDSHAKE_TIMEOUT,
    )
    .await
    {
        Ok(result) => {
            let info = server_info_from_result(&result);
            if !info.assessment.compatible {
                let why = info
                    .assessment
                    .warning
                    .clone()
                    .unwrap_or_else(|| "incompatible Music Assistant version".into());
                let _ = events.send(SessionNote::Server(info));
                return Err(why);
            }
            let _ = events.send(SessionNote::Server(info));
        }
        Err(e) => {
            tracing::warn!("music assistant info command failed: {e}");
            let _ = events.send(SessionNote::Server(ServerInfo {
                server_version: String::new(),
                schema_version: None,
                server_id: None,
                assessment: VersionAssessment {
                    compatible: true,
                    warning: Some(format!("could not read server info: {e}")),
                },
            }));
        }
    }

    let players_id = send_cmd(
        &mut socket,
        &mut next_id,
        &mut inflight,
        "players/all",
        json!({}),
        None,
    )
    .await?;
    let players_result = wait_result(
        &mut socket,
        &mut inflight,
        &events,
        &players_id,
        HANDSHAKE_TIMEOUT,
    )
    .await?;
    let _ = events.send(SessionNote::Players(players_from_result(&players_result)));

    let mut refresh = tokio::time::interval(PLAYER_REFRESH);
    refresh.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    refresh.tick().await;
    let mut commands_open = true;

    loop {
        tokio::select! {
            msg = socket.next() => {
                let Some(msg) = msg else {
                    return Err("Music Assistant closed the connection".into());
                };
                let msg = msg.map_err(|e| e.to_string())?;
                handle_socket_message(msg, &mut inflight, &events).await?;
            }
            cmd = outbound.recv(), if commands_open => {
                match cmd {
                    Some(cmd) => {
                        send_cmd(&mut socket, &mut next_id, &mut inflight, &cmd.command, cmd.args, cmd.reply).await?;
                    }
                    None => commands_open = false,
                }
            }
            _ = refresh.tick() => {
                send_cmd(&mut socket, &mut next_id, &mut inflight, "players/all", json!({}), None).await?;
            }
        }
    }
}

async fn handle_socket_message(
    msg: Message,
    inflight: &mut HashMap<String, Inflight>,
    events: &mpsc::UnboundedSender<SessionNote>,
) -> Result<(), String> {
    let text = match msg {
        Message::Text(t) => t.to_string(),
        Message::Close(_) => return Err("Music Assistant closed the connection".into()),
        Message::Ping(_) | Message::Pong(_) | Message::Binary(_) | Message::Frame(_) => {
            return Ok(())
        }
    };
    match parse_inbound(&text) {
        Inbound::Result { message_id, result } => {
            if let Some(pending) = inflight.remove(&message_id) {
                if let Some(reply) = pending.reply {
                    let _ = reply.send(Ok(result.clone()));
                }
            }
            // Unsolicited-looking players/all refresh uses a message id we sent
            // with reply None. Also surface a players list when the result is an
            // array of player objects so the periodic refresh updates registration.
            if result.as_array().is_some_and(|arr| {
                arr.first()
                    .and_then(|p| p.get("player_id"))
                    .and_then(|s| s.as_str())
                    .is_some()
            }) {
                let _ = events.send(SessionNote::Players(players_from_result(&result)));
            }
        }
        Inbound::Error {
            message_id,
            details,
            ..
        } => {
            if let Some(pending) = inflight.remove(&message_id) {
                if let Some(reply) = pending.reply {
                    let _ = reply.send(Err(details));
                }
            }
        }
        Inbound::Event {
            event,
            object_id,
            data,
        } => {
            let _ = events.send(SessionNote::Event {
                event,
                object_id,
                data,
            });
        }
        Inbound::Ignore => {}
    }
    Ok(())
}

async fn send_cmd<S>(
    socket: &mut S,
    next_id: &mut u64,
    inflight: &mut HashMap<String, Inflight>,
    command: &str,
    args: Value,
    reply: Option<oneshot::Sender<Result<Value, String>>>,
) -> Result<String, String>
where
    S: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    let id = next_id.to_string();
    *next_id += 1;
    let body = json!({
        "message_id": id,
        "command": command,
        "args": args,
    });
    socket
        .send(Message::Text(body.to_string().into()))
        .await
        .map_err(|e| format!("failed to send {command}: {e}"))?;
    inflight.insert(id.clone(), Inflight { reply });
    Ok(id)
}

async fn wait_result<S>(
    socket: &mut S,
    inflight: &mut HashMap<String, Inflight>,
    events: &mpsc::UnboundedSender<SessionNote>,
    message_id: &str,
    timeout: Duration,
) -> Result<Value, String>
where
    S: Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>
        + Sink<Message, Error = tokio_tungstenite::tungstenite::Error>
        + Unpin,
{
    let deadline = Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(format!(
                "timed out waiting for Music Assistant ({message_id})"
            ));
        }
        let msg = tokio::time::timeout(remaining, socket.next())
            .await
            .map_err(|_| "timed out waiting for Music Assistant".to_string())?
            .ok_or_else(|| "Music Assistant closed the connection".to_string())?
            .map_err(|e| e.to_string())?;
        let text = match msg {
            Message::Text(t) => t.to_string(),
            Message::Close(_) => return Err("Music Assistant closed the connection".into()),
            _ => continue,
        };
        match parse_inbound(&text) {
            Inbound::Result {
                message_id: id,
                result,
            } if id == message_id => {
                inflight.remove(&id);
                return Ok(result);
            }
            Inbound::Error {
                message_id: id,
                details,
                ..
            } if id == message_id => {
                inflight.remove(&id);
                return Err(details);
            }
            other => {
                // Reuse the live handler by reserializing would drop the match.
                // Dispatch directly so events during handshake are not lost.
                match other {
                    Inbound::Result {
                        message_id: id,
                        result,
                    } => {
                        if let Some(pending) = inflight.remove(&id) {
                            if let Some(reply) = pending.reply {
                                let _ = reply.send(Ok(result));
                            }
                        }
                    }
                    Inbound::Error {
                        message_id: id,
                        details,
                        ..
                    } => {
                        if let Some(pending) = inflight.remove(&id) {
                            if let Some(reply) = pending.reply {
                                let _ = reply.send(Err(details));
                            }
                        }
                    }
                    Inbound::Event {
                        event,
                        object_id,
                        data,
                    } => {
                        let _ = events.send(SessionNote::Event {
                            event,
                            object_id,
                            data,
                        });
                    }
                    Inbound::Ignore => {}
                }
            }
        }
    }
}

fn redact(err: String, token: &str) -> String {
    if token.is_empty() {
        err
    } else {
        err.replace(token, "[redacted]")
    }
}

#[derive(Debug, Clone)]
pub struct ProbeReport {
    pub ok: bool,
    pub detail: String,
    pub server_version: Option<String>,
    pub schema_version: Option<i64>,
    pub player_count: i64,
    pub warning: Option<String>,
}

pub async fn probe(ws_url: &str, token: &str) -> ProbeReport {
    let token_owned = token.to_string();
    let connect = tokio::time::timeout(
        Duration::from_secs(10),
        tokio_tungstenite::connect_async(ws_url),
    )
    .await;
    let socket = match connect {
        Ok(Ok((socket, _))) => socket,
        Ok(Err(e)) => {
            return ProbeReport::fail(redact(format!("could not connect: {e}"), token));
        }
        Err(_) => return ProbeReport::fail("timed out connecting to Music Assistant"),
    };
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
    let (note_tx, mut note_rx) = mpsc::unbounded_channel();
    let token_for_drive = token_owned.clone();
    let mut driver =
        tokio::spawn(async move { drive_socket(socket, &token_for_drive, cmd_rx, note_tx).await });
    let mut info: Option<ServerInfo> = None;
    let mut player_count: Option<i64> = None;
    let mut drive_err: Option<String> = None;
    let deadline = tokio::time::sleep(Duration::from_secs(15));
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            _ = &mut deadline => break,
            note = note_rx.recv() => {
                match note {
                    Some(SessionNote::Server(s)) => info = Some(s),
                    Some(SessionNote::Players(p)) => {
                        player_count = Some(p.len() as i64);
                        break;
                    }
                    Some(_) => {}
                    None => break,
                }
            }
            res = &mut driver => {
                drive_err = Some(match res {
                    Ok(Ok(())) => "connection closed".into(),
                    Ok(Err(e)) => e,
                    Err(e) => format!("connection task stopped: {e}"),
                });
                break;
            }
        }
    }
    drop(cmd_tx);
    driver.abort();

    if let Some(err) = drive_err {
        if player_count.is_none() {
            let warning = info.as_ref().and_then(|i| i.assessment.warning.clone());
            return ProbeReport {
                ok: false,
                detail: redact(err, &token_owned),
                server_version: info
                    .as_ref()
                    .map(|i| i.server_version.clone())
                    .filter(|s| !s.is_empty()),
                schema_version: info.as_ref().and_then(|i| i.schema_version),
                player_count: 0,
                warning,
            };
        }
    }
    let warning = info.as_ref().and_then(|i| i.assessment.warning.clone());
    let compatible = info
        .as_ref()
        .map(|i| i.assessment.compatible)
        .unwrap_or(true);
    let count = player_count.unwrap_or(0);
    if !compatible {
        return ProbeReport {
            ok: false,
            detail: warning
                .clone()
                .unwrap_or_else(|| "incompatible Music Assistant".into()),
            server_version: info.as_ref().map(|i| i.server_version.clone()),
            schema_version: info.as_ref().and_then(|i| i.schema_version),
            player_count: count,
            warning,
        };
    }
    ProbeReport {
        ok: true,
        detail: format!("connected, {count} player(s)"),
        server_version: info
            .as_ref()
            .map(|i| i.server_version.clone())
            .filter(|s| !s.is_empty()),
        schema_version: info.as_ref().and_then(|i| i.schema_version),
        player_count: count,
        warning,
    }
}

impl ProbeReport {
    fn fail(detail: impl Into<String>) -> Self {
        Self {
            ok: false,
            detail: detail.into(),
            server_version: None,
            schema_version: None,
            player_count: 0,
            warning: None,
        }
    }
}

// ============================ Settings =================================

#[derive(Debug, Clone)]
struct StoredSettings {
    enabled: bool,
    server_url: Option<String>,
    token_encrypted: Option<String>,
    external_base_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MaSettingsView {
    pub enabled: bool,
    pub server_url: Option<String>,
    pub external_base_url: Option<String>,
    pub has_token: bool,
    pub connected: bool,
    pub server_version: Option<String>,
    pub schema_version: Option<i64>,
    pub warning: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MaSettingsUpdate {
    pub user_id: i32,
    pub enabled: bool,
    pub server_url: Option<String>,
    pub external_base_url: Option<String>,
    pub token: Option<String>,
    pub clear_token: bool,
}

pub async fn load_settings_view(state: &AppState, user_id: i32) -> AppResult<MaSettingsView> {
    let stored = read_settings(&state.db_pool, user_id).await?;
    let live = state.music_assistant.status(user_id).await;
    Ok(view_from(&stored, &live))
}

fn view_from(stored: &StoredSettings, live: &SessionStatus) -> MaSettingsView {
    MaSettingsView {
        enabled: stored.enabled,
        server_url: stored.server_url.clone(),
        external_base_url: stored.external_base_url.clone(),
        has_token: stored
            .token_encrypted
            .as_deref()
            .map(|t| !t.is_empty())
            .unwrap_or(false),
        connected: live.connected,
        server_version: live.server_version.clone(),
        schema_version: live.schema_version,
        warning: live.warning.clone(),
        last_error: live.last_error.clone(),
    }
}

pub async fn save_settings(
    state: &AppState,
    update: MaSettingsUpdate,
) -> AppResult<MaSettingsView> {
    let existing = read_settings(&state.db_pool, update.user_id).await?;
    let mut server_url = match update
        .server_url
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(raw) => Some(parse_server_url(raw).map_err(AppError::bad_request)?.origin),
        None => existing.server_url.clone(),
    };
    let mut external = match update
        .external_base_url
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(raw) => Some(normalize_external_base(raw).map_err(AppError::bad_request)?),
        None => existing.external_base_url.clone(),
    };
    // Touch the JSON-RPC URL so a typo'd path fails at save time, and so the
    // field is not flagged unused when only the WebSocket is dialed.
    if let Some(raw) = server_url.as_deref() {
        let endpoints = parse_server_url(raw).map_err(AppError::bad_request)?;
        tracing::debug!("music assistant json-rpc endpoint {}", endpoints.api);
        server_url = Some(endpoints.origin);
    }
    let mut token_encrypted = existing.token_encrypted.clone();
    if update.clear_token {
        token_encrypted = None;
    }
    if let Some(token) = update
        .token
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        token_encrypted = Some(state.db_pool.encrypt_password(token).await?);
    }
    if update.enabled {
        if server_url.as_deref().unwrap_or("").is_empty() {
            return Err(AppError::bad_request(
                "server_url is required to enable Music Assistant",
            ));
        }
        if external.as_deref().unwrap_or("").is_empty() {
            return Err(AppError::bad_request(
                "external_base_url is required so Music Assistant can reach PinePods",
            ));
        }
        if token_encrypted.as_deref().unwrap_or("").is_empty() {
            return Err(AppError::bad_request(
                "a Music Assistant token is required to enable the connection",
            ));
        }
    }
    if !update.enabled {
        // Keep previously validated values when the client omits them.
        let _ = (&mut server_url, &mut external);
    }
    write_settings(
        &state.db_pool,
        update.user_id,
        StoredSettings {
            enabled: update.enabled,
            server_url,
            token_encrypted,
            external_base_url: external,
        },
    )
    .await?;
    state.music_assistant.request_reload();
    load_settings_view(state, update.user_id).await
}

pub async fn test_connection(
    state: &AppState,
    user_id: i32,
    server_url: Option<&str>,
    token: Option<&str>,
) -> AppResult<ProbeReport> {
    let stored = read_settings(&state.db_pool, user_id).await?;
    let url_raw = server_url
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or(stored.server_url.clone());
    let Some(url_raw) = url_raw else {
        return Ok(ProbeReport::fail(
            "Music Assistant is not configured. Provide a server URL and token.",
        ));
    };
    let endpoints = parse_server_url(&url_raw).map_err(AppError::bad_request)?;
    let token_owned = if let Some(t) = token.map(str::trim).filter(|t| !t.is_empty()) {
        t.to_string()
    } else if let Some(enc) = stored.token_encrypted.as_deref().filter(|t| !t.is_empty()) {
        state.db_pool.decrypt_password(enc).await?
    } else {
        return Ok(ProbeReport::fail(
            "Music Assistant is not configured. Provide a server URL and token.",
        ));
    };
    Ok(probe(&endpoints.ws, &token_owned).await)
}

async fn read_settings(db: &DatabasePool, user_id: i32) -> AppResult<StoredSettings> {
    match db {
        DatabasePool::Postgres(pool) => {
            let row = sqlx::query(
                r#"SELECT CASE WHEN enabled THEN 1 ELSE 0 END AS enabled,
                          serverurl AS server_url,
                          tokenencrypted AS token_encrypted,
                          externalbaseurl AS external_base_url
                   FROM "MusicAssistant" WHERE userid = $1"#,
            )
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
            Ok(match row {
                Some(row) => map_stored(
                    row.try_get::<i32, _>("enabled")? as i64,
                    row.try_get("server_url")?,
                    row.try_get("token_encrypted")?,
                    row.try_get("external_base_url")?,
                ),
                None => StoredSettings::disabled(),
            })
        }
        DatabasePool::MySQL(pool) => {
            let row = sqlx::query(
                r#"SELECT IF(Enabled, 1, 0) AS enabled,
                          ServerUrl AS server_url,
                          TokenEncrypted AS token_encrypted,
                          ExternalBaseUrl AS external_base_url
                   FROM MusicAssistant WHERE UserID = ?"#,
            )
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
            Ok(match row {
                Some(row) => {
                    let enabled: i64 = row
                        .try_get::<i64, _>("enabled")
                        .or_else(|_| row.try_get::<i32, _>("enabled").map(|v| v as i64))?;
                    map_stored(
                        enabled,
                        row.try_get("server_url")?,
                        row.try_get("token_encrypted")?,
                        row.try_get("external_base_url")?,
                    )
                }
                None => StoredSettings::disabled(),
            })
        }
    }
}

impl StoredSettings {
    fn disabled() -> Self {
        Self {
            enabled: false,
            server_url: None,
            token_encrypted: None,
            external_base_url: None,
        }
    }
}

fn map_stored(
    enabled: i64,
    server_url: Option<String>,
    token: Option<String>,
    base: Option<String>,
) -> StoredSettings {
    let clean = |v: Option<String>| v.filter(|s| !s.is_empty());
    StoredSettings {
        enabled: enabled != 0,
        server_url: clean(server_url),
        token_encrypted: clean(token),
        external_base_url: clean(base),
    }
}

async fn write_settings(db: &DatabasePool, user_id: i32, stored: StoredSettings) -> AppResult<()> {
    match db {
        DatabasePool::Postgres(pool) => {
            sqlx::query(
                r#"INSERT INTO "MusicAssistant" (userid, enabled, serverurl, tokenencrypted, externalbaseurl, updatedat)
                   VALUES ($1, $2, $3, $4, $5, NOW())
                   ON CONFLICT (userid) DO UPDATE SET
                     enabled = EXCLUDED.enabled,
                     serverurl = EXCLUDED.serverurl,
                     tokenencrypted = EXCLUDED.tokenencrypted,
                     externalbaseurl = EXCLUDED.externalbaseurl,
                     updatedat = NOW()"#,
            )
            .bind(user_id)
            .bind(stored.enabled)
            .bind(&stored.server_url)
            .bind(&stored.token_encrypted)
            .bind(&stored.external_base_url)
            .execute(pool)
            .await?;
        }
        DatabasePool::MySQL(pool) => {
            sqlx::query(
                r#"INSERT INTO MusicAssistant (UserID, Enabled, ServerUrl, TokenEncrypted, ExternalBaseUrl, UpdatedAt)
                   VALUES (?, ?, ?, ?, ?, NOW())
                   ON DUPLICATE KEY UPDATE
                     Enabled = VALUES(Enabled),
                     ServerUrl = VALUES(ServerUrl),
                     TokenEncrypted = VALUES(TokenEncrypted),
                     ExternalBaseUrl = VALUES(ExternalBaseUrl),
                     UpdatedAt = NOW()"#,
            )
            .bind(user_id)
            .bind(stored.enabled)
            .bind(&stored.server_url)
            .bind(&stored.token_encrypted)
            .bind(&stored.external_base_url)
            .execute(pool)
            .await?;
        }
    }
    Ok(())
}

async fn list_enabled(db: &DatabasePool) -> AppResult<Vec<(i32, StoredSettings)>> {
    match db {
        DatabasePool::Postgres(pool) => {
            let rows = sqlx::query(
                r#"SELECT userid AS user_id,
                          CASE WHEN enabled THEN 1 ELSE 0 END AS enabled,
                          serverurl AS server_url,
                          tokenencrypted AS token_encrypted,
                          externalbaseurl AS external_base_url
                   FROM "MusicAssistant"
                   WHERE enabled = TRUE
                     AND serverurl IS NOT NULL AND serverurl <> ''
                     AND tokenencrypted IS NOT NULL AND tokenencrypted <> ''
                     AND externalbaseurl IS NOT NULL AND externalbaseurl <> ''"#,
            )
            .fetch_all(pool)
            .await?;
            let mut out = Vec::new();
            for row in rows {
                let user_id: i32 = row.try_get("user_id")?;
                let enabled: i32 = row.try_get("enabled")?;
                out.push((
                    user_id,
                    map_stored(
                        enabled as i64,
                        row.try_get("server_url")?,
                        row.try_get("token_encrypted")?,
                        row.try_get("external_base_url")?,
                    ),
                ));
            }
            Ok(out)
        }
        DatabasePool::MySQL(pool) => {
            let rows = sqlx::query(
                r#"SELECT UserID AS user_id,
                          IF(Enabled, 1, 0) AS enabled,
                          ServerUrl AS server_url,
                          TokenEncrypted AS token_encrypted,
                          ExternalBaseUrl AS external_base_url
                   FROM MusicAssistant
                   WHERE Enabled = 1
                     AND ServerUrl IS NOT NULL AND ServerUrl <> ''
                     AND TokenEncrypted IS NOT NULL AND TokenEncrypted <> ''
                     AND ExternalBaseUrl IS NOT NULL AND ExternalBaseUrl <> ''"#,
            )
            .fetch_all(pool)
            .await?;
            let mut out = Vec::new();
            for row in rows {
                let user_id: i32 = match row.try_get("user_id") {
                    Ok(v) => v,
                    Err(_) => row.try_get::<i64, _>("user_id")? as i32,
                };
                let enabled: i64 = row
                    .try_get::<i64, _>("enabled")
                    .or_else(|_| row.try_get::<i32, _>("enabled").map(|v| v as i64))?;
                out.push((
                    user_id,
                    map_stored(
                        enabled,
                        row.try_get("server_url")?,
                        row.try_get("token_encrypted")?,
                        row.try_get("external_base_url")?,
                    ),
                ));
            }
            Ok(out)
        }
    }
}

// ============================ Hub and session ============================

#[derive(Debug, Clone)]
pub struct ActionOutcome {
    pub detail: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SessionStatus {
    pub connected: bool,
    pub server_version: Option<String>,
    pub schema_version: Option<i64>,
    pub warning: Option<String>,
    pub last_error: Option<String>,
}

struct SessionLive {
    connected: AtomicBool,
    players: RwLock<Vec<MaPlayer>>,
    server_version: RwLock<Option<String>>,
    schema_version: RwLock<Option<i64>>,
    warning: RwLock<Option<String>>,
    last_error: RwLock<Option<String>>,
}

impl SessionLive {
    fn new() -> Self {
        Self {
            connected: AtomicBool::new(false),
            players: RwLock::new(Vec::new()),
            server_version: RwLock::new(None),
            schema_version: RwLock::new(None),
            warning: RwLock::new(None),
            last_error: RwLock::new(None),
        }
    }
}

struct SessionSlot {
    outbound: mpsc::UnboundedSender<Outbound>,
    live: Arc<SessionLive>,
    fingerprint: String,
    cancel: watch::Sender<bool>,
}

struct HubInner {
    sessions: RwLock<HashMap<i32, SessionSlot>>,
    reload: Notify,
}

#[derive(Clone)]
pub struct MusicAssistantHub {
    inner: Arc<HubInner>,
}

impl MusicAssistantHub {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(HubInner {
                sessions: RwLock::new(HashMap::new()),
                reload: Notify::new(),
            }),
        }
    }

    pub fn request_reload(&self) {
        self.inner.reload.notify_waiters();
    }

    pub async fn status(&self, user_id: i32) -> SessionStatus {
        let sessions = self.inner.sessions.read().await;
        let Some(slot) = sessions.get(&user_id) else {
            return SessionStatus::default();
        };
        let connected = slot.live.connected.load(Ordering::Relaxed);
        let server_version = slot.live.server_version.read().await.clone();
        let schema_version = *slot.live.schema_version.read().await;
        let warning = slot.live.warning.read().await.clone();
        let last_error = slot.live.last_error.read().await.clone();
        drop(sessions);
        SessionStatus {
            connected,
            server_version,
            schema_version,
            warning,
            last_error,
        }
    }

    pub async fn players(&self, user_id: i32) -> Vec<MaPlayer> {
        let sessions = self.inner.sessions.read().await;
        let Some(slot) = sessions.get(&user_id) else {
            return Vec::new();
        };
        let players = slot.live.players.read().await.clone();
        drop(sessions);
        players
    }

    async fn send_commands(
        &self,
        user_id: i32,
        commands: Vec<MaCommand>,
    ) -> AppResult<ActionOutcome> {
        let tx = {
            let sessions = self.inner.sessions.read().await;
            sessions
                .get(&user_id)
                .map(|s| s.outbound.clone())
                .ok_or_else(|| AppError::service_unavailable("Music Assistant is not connected"))?
        };
        let mut warnings = Vec::new();
        for (i, cmd) in commands.into_iter().enumerate() {
            let (reply_tx, reply_rx) = oneshot::channel();
            let name = cmd.command.clone();
            tx.send(Outbound {
                command: cmd.command,
                args: cmd.args,
                reply: Some(reply_tx),
            })
            .map_err(|_| AppError::service_unavailable("Music Assistant is not connected"))?;
            let result = tokio::time::timeout(COMMAND_TIMEOUT, reply_rx)
                .await
                .map_err(|_| AppError::service_unavailable("Music Assistant did not respond"))?
                .map_err(|_| AppError::service_unavailable("Music Assistant connection lost"))?;
            match result {
                Ok(_) => {}
                Err(e) if i == 0 => return Err(AppError::bad_request(e)),
                Err(e) => warnings.push(format!("{name}: {e}")),
            }
        }
        Ok(ActionOutcome {
            detail: "ok".into(),
            warnings,
        })
    }
}

impl Default for MusicAssistantHub {
    fn default() -> Self {
        Self::new()
    }
}

/// Apply a relay or REST action for one of the user's Music Assistant players.
pub async fn dispatch(
    state: &AppState,
    user_id: i32,
    player_or_device: &str,
    action: &str,
    args: &Value,
) -> AppResult<ActionOutcome> {
    let player_id = canonical_player_id(user_id, player_or_device);
    if player_id.is_empty() {
        return Err(AppError::bad_request("player_id is required"));
    }
    let args = canonicalize_args(user_id, args);
    let players = state.music_assistant.players(user_id).await;
    let queue_id = queue_id_for(&players, &player_id);
    let commands = if action == "play_episode" {
        prepare_play_episode(state, user_id, &queue_id, &args).await?
    } else {
        translate_action(&player_id, &queue_id, action, &args).map_err(AppError::bad_request)?
    };
    state.music_assistant.send_commands(user_id, commands).await
}

async fn prepare_play_episode(
    state: &AppState,
    user_id: i32,
    queue_id: &str,
    args: &Value,
) -> AppResult<Vec<MaCommand>> {
    let episode_id = args
        .get("episode_id")
        .and_then(json_i64)
        .ok_or_else(|| AppError::bad_request("play_episode requires episode_id"))?;
    if episode_id <= 0 || episode_id > i32::MAX as i64 {
        return Err(AppError::bad_request("play_episode requires episode_id"));
    }
    let episode_id = episode_id as i32;
    let is_youtube = args
        .get("is_youtube")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let meta = state
        .db_pool
        .get_episode_metadata(episode_id, user_id, false, is_youtube)
        .await?;
    let podcast_id = meta
        .get("podcastid")
        .and_then(json_i64)
        .ok_or_else(|| AppError::not_found("Episode not found"))? as i32;
    let stored = read_settings(&state.db_pool, user_id).await?;
    let base = stored
        .external_base_url
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            AppError::bad_request("Set an external base URL so Music Assistant can reach PinePods")
        })?;
    let rss_key = state
        .db_pool
        .get_or_create_podcast_scoped_rss_key(user_id, podcast_id)
        .await?;
    let downloaded = if is_youtube {
        state
            .db_pool
            .get_youtube_video_location(episode_id, user_id)
            .await
            .ok()
            .flatten()
    } else {
        state
            .db_pool
            .get_download_location(episode_id, user_id)
            .await
            .ok()
            .flatten()
    };
    let prefer = args
        .get("media_option_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());
    // A saved file is served by PinePods. Otherwise plan a remote enclosure.
    // Swallowing a planner error used to fall through to the stream URL, which
    // 404s until the episode is downloaded.
    let plan = if is_youtube || downloaded.is_some() {
        None
    } else {
        match crate::services::alternate_media::plan_for_episode(
            &state.db_pool,
            episode_id,
            user_id,
            prefer,
            crate::services::alternate_media::Audience::MusicAssistant,
        )
        .await
        {
            Ok(plan) => Some(plan),
            Err(e) => {
                tracing::warn!(
                    "music assistant media plan failed for episode {episode_id} user {user_id}: {e}"
                );
                return Err(e);
            }
        }
    };
    let served_by_pinepods = downloaded.is_some()
        || plan.as_ref().is_some_and(|plan| {
            plan.options.iter().any(|opt| {
                opt.id == "primary"
                    && opt
                        .sources
                        .iter()
                        .any(|src| src.uri.trim().to_ascii_lowercase().starts_with("local://"))
            })
        });
    let direct = plan.and_then(|plan| plan.playback.into_iter().next().map(|step| step.uri));
    // Feed username/password is not applied to a direct URL; download the episode
    // first when the host requires those credentials.
    let media = match ma_playback(served_by_pinepods, direct) {
        Ok(MaPlayback::Direct(uri)) => uri,
        Ok(MaPlayback::Stream) => build_stream_url(base, episode_id, &rss_key, user_id, is_youtube),
        Err(msg) => {
            tracing::warn!("music assistant episode {episode_id} user {user_id}: {msg}");
            return Err(AppError::bad_request(msg));
        }
    };
    let position = if args.get("position_sec").is_some() {
        args.get("position_sec").and_then(json_f64).unwrap_or(0.0)
    } else {
        meta.get("listenduration").and_then(json_f64).unwrap_or(0.0)
    };
    let speed = state
        .db_pool
        .get_playback_speed(user_id, is_youtube, Some(podcast_id))
        .await
        .unwrap_or(1.0);
    let speed = if speed > 0.0 { speed } else { 1.0 };
    Ok(play_media_commands(queue_id, &media, position, speed))
}

pub fn spawn_supervisor(state: AppState) {
    tokio::spawn(async move {
        loop {
            if let Err(e) = reconcile(&state).await {
                tracing::warn!("music assistant reconcile failed: {e}");
            }
            tokio::select! {
                _ = state.music_assistant.inner.reload.notified() => {}
                _ = tokio::time::sleep(Duration::from_secs(30)) => {}
            }
        }
    });
}

async fn reconcile(state: &AppState) -> AppResult<()> {
    let enabled = list_enabled(&state.db_pool).await?;
    let mut wanted: HashMap<i32, (String, StoredSettings)> = HashMap::new();
    for (user_id, stored) in enabled {
        let fp = fingerprint(&stored);
        wanted.insert(user_id, (fp, stored));
    }
    let mut stop = Vec::new();
    {
        let sessions = state.music_assistant.inner.sessions.read().await;
        for (user_id, slot) in sessions.iter() {
            match wanted.get(user_id) {
                Some((fp, _)) if fp == &slot.fingerprint => {}
                _ => stop.push(*user_id),
            }
        }
    }
    for user_id in stop {
        if let Some(slot) = state
            .music_assistant
            .inner
            .sessions
            .write()
            .await
            .remove(&user_id)
        {
            let _ = slot.cancel.send(true);
        }
    }
    for (user_id, (fp, stored)) in wanted {
        if state
            .music_assistant
            .inner
            .sessions
            .read()
            .await
            .contains_key(&user_id)
        {
            continue;
        }
        let token = match state
            .db_pool
            .decrypt_password(stored.token_encrypted.as_deref().unwrap_or(""))
            .await
        {
            Ok(t) if !t.is_empty() => t,
            Ok(_) => {
                tracing::warn!("music assistant token for user {user_id} decrypted empty");
                continue;
            }
            Err(e) => {
                tracing::warn!(
                    "music assistant token for user {user_id} could not be decrypted: {e}"
                );
                continue;
            }
        };
        let endpoints = match parse_server_url(stored.server_url.as_deref().unwrap_or("")) {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!("music assistant URL for user {user_id} is invalid: {e}");
                continue;
            }
        };
        let external = stored.external_base_url.clone().unwrap_or_default();
        let (tx, rx) = mpsc::unbounded_channel();
        let (cancel_tx, cancel_rx) = watch::channel(false);
        let live = Arc::new(SessionLive::new());
        state.music_assistant.inner.sessions.write().await.insert(
            user_id,
            SessionSlot {
                outbound: tx,
                live: live.clone(),
                fingerprint: fp,
                cancel: cancel_tx,
            },
        );
        let state = state.clone();
        tokio::spawn(async move {
            maintain(
                state, user_id, endpoints, external, token, rx, live, cancel_rx,
            )
            .await;
        });
    }
    Ok(())
}

fn fingerprint(stored: &StoredSettings) -> String {
    format!(
        "{}|{}|{}",
        stored.server_url.as_deref().unwrap_or(""),
        stored.external_base_url.as_deref().unwrap_or(""),
        stored.token_encrypted.as_deref().unwrap_or("")
    )
}

struct QueueTrack {
    stream: Option<StreamRef>,
    title: String,
    artwork: String,
    duration: f64,
    position: f64,
    playing: bool,
    speed: f64,
    sample: Option<ProgressSample>,
    last_write: Option<Instant>,
    loaded_episode: Option<(i32, bool)>,
}

async fn maintain(
    state: AppState,
    user_id: i32,
    endpoints: MaEndpoints,
    external_base: String,
    token: String,
    mut outbound: mpsc::UnboundedReceiver<Outbound>,
    live: Arc<SessionLive>,
    mut cancel: watch::Receiver<bool>,
) {
    let mut virtuals: HashMap<String, u64> = HashMap::new();
    let mut tracks: HashMap<String, QueueTrack> = HashMap::new();
    let mut backoff = 2u64;
    loop {
        if *cancel.borrow() {
            break;
        }
        match run_once(
            &state,
            user_id,
            &endpoints,
            &external_base,
            &token,
            &mut outbound,
            &live,
            &mut cancel,
            &mut virtuals,
            &mut tracks,
        )
        .await
        {
            Ok(()) => backoff = 2,
            Err(e) => {
                let redacted = redact(e, &token);
                tracing::warn!("music assistant user {user_id}: {redacted}");
                *live.last_error.write().await = Some(redacted);
                live.connected.store(false, Ordering::Relaxed);
            }
        }
        if *cancel.borrow() || outbound.is_closed() {
            break;
        }
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(backoff)) => {}
            _ = cancel.changed() => {}
        }
        if *cancel.borrow() {
            break;
        }
        backoff = (backoff * 2).min(60);
    }
    live.connected.store(false, Ordering::Relaxed);
    clear_virtuals(&state, user_id, &mut virtuals).await;
}

async fn run_once(
    state: &AppState,
    user_id: i32,
    endpoints: &MaEndpoints,
    _external_base: &str,
    token: &str,
    outbound: &mut mpsc::UnboundedReceiver<Outbound>,
    live: &Arc<SessionLive>,
    cancel: &mut watch::Receiver<bool>,
    virtuals: &mut HashMap<String, u64>,
    tracks: &mut HashMap<String, QueueTrack>,
) -> Result<(), String> {
    let socket = tokio::time::timeout(
        Duration::from_secs(10),
        tokio_tungstenite::connect_async(&endpoints.ws),
    )
    .await
    .map_err(|_| "timed out connecting to Music Assistant".to_string())?
    .map_err(|e| redact(e.to_string(), token))?
    .0;
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
    let (note_tx, mut note_rx) = mpsc::unbounded_channel();
    let token_for_drive = token.to_string();
    let mut driver =
        tokio::spawn(async move { drive_socket(socket, &token_for_drive, cmd_rx, note_tx).await });
    live.connected.store(true, Ordering::Relaxed);
    let result = loop {
        tokio::select! {
            res = &mut driver => {
                live.connected.store(false, Ordering::Relaxed);
                break match res {
                    Ok(r) => r,
                    Err(e) => Err(format!("connection task stopped: {e}")),
                };
            }
            _ = cancel.changed() => {
                if *cancel.borrow() {
                    break Ok(());
                }
            }
            note = note_rx.recv() => {
                match note {
                    Some(note) => apply_note(state, user_id, live, virtuals, tracks, note).await,
                    None => break Err("Music Assistant connection ended".into()),
                }
            }
            cmd = outbound.recv() => {
                match cmd {
                    Some(cmd) => {
                        if cmd_tx.send(cmd).is_err() {
                            break Err("Music Assistant connection ended".into());
                        }
                    }
                    None => break Ok(()),
                }
            }
        }
    };
    live.connected.store(false, Ordering::Relaxed);
    driver.abort();
    result
}

async fn apply_note(
    state: &AppState,
    user_id: i32,
    live: &Arc<SessionLive>,
    virtuals: &mut HashMap<String, u64>,
    tracks: &mut HashMap<String, QueueTrack>,
    note: SessionNote,
) {
    match note {
        SessionNote::Server(info) => {
            *live.server_version.write().await =
                Some(info.server_version).filter(|s| !s.is_empty());
            *live.schema_version.write().await = info.schema_version;
            *live.warning.write().await = info.assessment.warning;
        }
        SessionNote::Players(players) => {
            *live.players.write().await = players.clone();
            if let Err(e) = sync_virtuals(state, user_id, &players, virtuals, tracks).await {
                tracing::debug!("music assistant device sync: {e}");
            }
        }
        SessionNote::Event {
            event,
            object_id,
            data,
        } => {
            if event == "player_updated" || event == "player_added" {
                if let Some(player) = player_from_value(&data) {
                    let mut players = live.players.write().await;
                    if let Some(slot) = players.iter_mut().find(|p| p.player_id == player.player_id)
                    {
                        *slot = player;
                    } else if event == "player_added" {
                        players.push(player);
                    }
                    let snapshot_players = players.clone();
                    drop(players);
                    let _ =
                        sync_virtuals(state, user_id, &snapshot_players, virtuals, tracks).await;
                }
            } else if event == "player_removed" {
                if let Some(id) = object_id.or_else(|| {
                    data.get("player_id")
                        .and_then(|s| s.as_str())
                        .map(|s| s.to_string())
                }) {
                    live.players.write().await.retain(|p| p.player_id != id);
                    let players = live.players.read().await.clone();
                    let _ = sync_virtuals(state, user_id, &players, virtuals, tracks).await;
                }
            } else if event.starts_with("queue_") {
                let mut update = progress_from_payload(&data);
                if update.queue_id.is_none() {
                    update.queue_id = object_id;
                }
                apply_progress(state, user_id, live, tracks, update).await;
            }
        }
    }
}

async fn apply_progress(
    state: &AppState,
    user_id: i32,
    live: &Arc<SessionLive>,
    tracks: &mut HashMap<String, QueueTrack>,
    update: ProgressUpdate,
) {
    let Some(queue_id) = update.queue_id.clone() else {
        return;
    };
    let track = tracks
        .entry(queue_id.clone())
        .or_insert_with(|| QueueTrack {
            stream: None,
            title: String::new(),
            artwork: String::new(),
            duration: 0.0,
            position: 0.0,
            playing: false,
            speed: 1.0,
            sample: None,
            last_write: None,
            loaded_episode: None,
        });
    let episode_changed = update.stream.as_ref().map(|s| s.episode_id)
        != track.stream.as_ref().map(|s| s.episode_id)
        && update.stream.is_some();
    if let Some(stream) = update.stream.clone() {
        track.stream = Some(stream);
    }
    if let Some(p) = update.position_sec {
        track.position = p.max(0.0);
    }
    if let Some(d) = update.duration_sec {
        if d > 0.0 {
            track.duration = d;
        }
    }
    if let Some(playing) = update.playing {
        track.playing = playing;
    }
    if let Some(title) = update.title.clone() {
        if !title.is_empty() && track.title.is_empty() {
            track.title = title;
        }
    }
    if let Some(speed) = update.speed {
        if speed > 0.0 {
            track.speed = speed;
        }
    }
    let Some(stream) = track.stream.clone() else {
        publish_track_snapshot(state, user_id, live, &queue_id, track).await;
        return;
    };
    if stream.user_id.is_some_and(|id| id != user_id) {
        return;
    }
    if episode_changed || track.loaded_episode != Some((stream.episode_id, stream.is_youtube)) {
        if let Ok(meta) = state
            .db_pool
            .get_episode_metadata(stream.episode_id, user_id, false, stream.is_youtube)
            .await
        {
            if let Some(title) = meta.get("episodetitle").and_then(|v| v.as_str()) {
                track.title = title.to_string();
            }
            if let Some(art) = meta.get("episodeartwork").and_then(|v| v.as_str()) {
                track.artwork = art.to_string();
            }
            if let Some(dur) = meta.get("episodeduration").and_then(json_f64) {
                if dur > 0.0 && track.duration <= 0.0 {
                    track.duration = dur;
                }
            }
            track.loaded_episode = Some((stream.episode_id, stream.is_youtube));
        }
    }
    let sample = ProgressSample {
        episode_id: stream.episode_id,
        position_sec: track.position,
        playing: track.playing,
    };
    let since = track
        .last_write
        .map(|t| t.elapsed())
        .unwrap_or(PROGRESS_WRITE_EVERY);
    if should_write_progress(track.sample.as_ref(), &sample, since) {
        write_listen_progress(
            state,
            user_id,
            stream.episode_id,
            stream.is_youtube,
            track.position,
            track.duration,
        )
        .await;
        track.last_write = Some(Instant::now());
        track.sample = Some(sample);
    }
    publish_track_snapshot(state, user_id, live, &queue_id, track).await;
}

async fn write_listen_progress(
    state: &AppState,
    user_id: i32,
    episode_id: i32,
    is_youtube: bool,
    position: f64,
    duration_hint: f64,
) {
    if let Err(e) = state
        .db_pool
        .record_podcast_history(episode_id, user_id, position as f32, is_youtube)
        .await
    {
        tracing::debug!("music assistant progress write failed: {e}");
        return;
    }
    let auto = state
        .db_pool
        .get_user_auto_complete_seconds(user_id)
        .await
        .unwrap_or(0);
    if auto <= 0 {
        return;
    }
    let duration = if duration_hint > 0.0 {
        duration_hint as i32
    } else if is_youtube {
        state
            .db_pool
            .get_youtube_episode_duration(episode_id)
            .await
            .unwrap_or(0)
    } else {
        state
            .db_pool
            .get_episode_duration(episode_id)
            .await
            .unwrap_or(0)
    };
    if duration > 0 {
        let remaining = duration as f64 - position;
        if remaining <= auto as f64 || position >= duration as f64 {
            let _ = state
                .db_pool
                .mark_episode_completed(episode_id, user_id, is_youtube)
                .await;
        }
    }
}

async fn publish_track_snapshot(
    state: &AppState,
    user_id: i32,
    live: &Arc<SessionLive>,
    queue_id: &str,
    track: &QueueTrack,
) {
    let players = live.players.read().await;
    let Some(player) = players
        .iter()
        .find(|p| p.player_id == queue_id || p.active_source.as_deref() == Some(queue_id))
    else {
        return;
    };
    let snap = snapshot_for(user_id, player, Some(track));
    drop(players);
    if state
        .redis_client
        .upsert_now_playing(user_id, snap)
        .await
        .is_ok()
    {
        notify_devices_changed(state, user_id, "").await;
    }
}

async fn sync_virtuals(
    state: &AppState,
    user_id: i32,
    players: &[MaPlayer],
    virtuals: &mut HashMap<String, u64>,
    tracks: &HashMap<String, QueueTrack>,
) -> AppResult<()> {
    let mut keep = HashSet::new();
    for player in players.iter().filter(|p| p.available) {
        let device_id = device_id_for(user_id, &player.player_id);
        keep.insert(device_id.clone());
        if !virtuals.contains_key(&device_id) {
            let (tx, mut rx) = mpsc::unbounded_channel();
            let conn = state
                .now_playing_manager
                .register_virtual(user_id, device_id.clone(), tx)
                .await;
            virtuals.insert(device_id.clone(), conn);
            let state = state.clone();
            let player_id = player.player_id.clone();
            tokio::spawn(async move {
                while let Some(msg) = rx.recv().await {
                    let text = match msg {
                        RelayMessage::Text(t) => t.to_string(),
                        RelayMessage::Close(_) => break,
                        _ => continue,
                    };
                    let Some((action, args)) = parse_relay_command(&text) else {
                        continue;
                    };
                    if let Err(e) = dispatch(&state, user_id, &player_id, &action, &args).await {
                        tracing::warn!("music assistant {player_id} {action} failed: {e}");
                    }
                }
            });
        }
        let queue_key = player.active_source.as_deref().unwrap_or(&player.player_id);
        let track = tracks
            .get(queue_key)
            .or_else(|| tracks.get(&player.player_id));
        let snap = snapshot_for(user_id, player, track);
        state.redis_client.upsert_now_playing(user_id, snap).await?;
    }
    let stale: Vec<String> = virtuals
        .keys()
        .filter(|id| !keep.contains(*id))
        .cloned()
        .collect();
    for device_id in stale {
        if let Some(conn) = virtuals.remove(&device_id) {
            state
                .now_playing_manager
                .unregister_virtual(user_id, &device_id, conn)
                .await;
        }
        state
            .redis_client
            .remove_now_playing(user_id, &device_id)
            .await?;
    }
    notify_devices_changed(state, user_id, "").await;
    Ok(())
}

async fn clear_virtuals(state: &AppState, user_id: i32, virtuals: &mut HashMap<String, u64>) {
    for (device_id, conn) in virtuals.drain() {
        state
            .now_playing_manager
            .unregister_virtual(user_id, &device_id, conn)
            .await;
        let _ = state
            .redis_client
            .remove_now_playing(user_id, &device_id)
            .await;
    }
    notify_devices_changed(state, user_id, "").await;
}

fn snapshot_for(user_id: i32, player: &MaPlayer, track: Option<&QueueTrack>) -> NowPlayingSnapshot {
    let (episode_id, is_youtube, title, artwork, position, duration, playing, speed) = match track {
        Some(t) => {
            let (episode_id, is_youtube) = t
                .stream
                .as_ref()
                .map(|s| (s.episode_id, s.is_youtube))
                .unwrap_or((0, false));
            let title = if t.title.is_empty() {
                player.current_title.clone().unwrap_or_default()
            } else {
                t.title.clone()
            };
            (
                episode_id,
                is_youtube,
                title,
                t.artwork.clone(),
                t.position,
                if t.duration > 0.0 { t.duration } else { 0.0 },
                t.playing,
                t.speed,
            )
        }
        None => (
            0,
            false,
            player.current_title.clone().unwrap_or_default(),
            String::new(),
            player.elapsed_time.unwrap_or(0.0),
            0.0,
            player.state == "playing" || player.state == "buffering",
            1.0,
        ),
    };
    NowPlayingSnapshot {
        device_id: device_id_for(user_id, &player.player_id),
        device_name: player.name.clone(),
        device_type: DEVICE_TYPE.to_string(),
        episode_id,
        is_youtube,
        title,
        artwork_url: artwork,
        position_sec: position,
        duration_sec: duration,
        playing,
        speed,
        updated_at: 0,
        capabilities: MA_CAPABILITIES.iter().map(|s| (*s).to_string()).collect(),
        volume_level: player.volume_level,
        volume_muted: Some(player.volume_muted),
        ma_player_id: Some(player.player_id.clone()),
        group_members: Some(player.group_members.clone()),
    }
}

fn json_f64(v: &Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| v.as_i64().map(|n| n as f64))
        .or_else(|| v.as_u64().map(|n| n as f64))
}

fn json_i64(v: &Value) -> Option<i64> {
    v.as_i64()
        .or_else(|| v.as_u64().map(|n| n as i64))
        .or_else(|| v.as_f64().map(|n| n as i64))
}

// ============================ Tests ======================================

#[cfg(test)]
mod tests {
    use super::*;
    use futures::{SinkExt, StreamExt};
    use tokio::io::DuplexStream;
    use tokio_tungstenite::tungstenite::protocol::Role;
    use tokio_tungstenite::WebSocketStream;

    #[test]
    fn external_base_rejects_loopback_and_accepts_lan() {
        assert!(normalize_external_base("http://127.0.0.1:8040").is_err());
        assert!(normalize_external_base("http://localhost:8040").is_err());
        assert!(normalize_external_base("http://[::1]:8040").is_err());
        assert!(normalize_external_base("http://169.254.169.254/").is_err());
        assert_eq!(
            normalize_external_base("http://192.168.1.20:8040/").unwrap(),
            "http://192.168.1.20:8040"
        );
        assert_eq!(
            normalize_external_base("https://pods.example.com").unwrap(),
            "https://pods.example.com"
        );
        assert!(normalize_external_base("http://192.168.1.20:8040/pinepods").is_err());
    }

    #[test]
    fn server_url_normalizes_ws_and_blocks_metadata() {
        let ep = parse_server_url("http://192.168.1.5:8095").unwrap();
        assert_eq!(ep.ws, "ws://192.168.1.5:8095/ws");
        assert_eq!(ep.api, "http://192.168.1.5:8095/api");
        assert_eq!(ep.origin, "http://192.168.1.5:8095");
        let via_ws = parse_server_url("ws://192.168.1.5:8095/ws").unwrap();
        assert_eq!(via_ws.ws, ep.ws);
        assert!(parse_server_url("http://127.0.0.1:8095").is_ok());
        assert!(parse_server_url("http://169.254.169.254").is_err());
        assert!(parse_server_url("file:///etc/passwd").is_err());
        let def = parse_server_url("http://ma.home").unwrap();
        assert_eq!(def.ws, "ws://ma.home:8095/ws");
    }

    #[test]
    fn undownloaded_episode_uses_the_enclosure_and_never_a_missing_stream() {
        assert_eq!(
            ma_playback(false, Some("https://cdn.example/ep.mp3".into())).unwrap(),
            MaPlayback::Direct("https://cdn.example/ep.mp3".into())
        );
        assert_eq!(ma_playback(true, None).unwrap(), MaPlayback::Stream);
        assert!(ma_playback(false, None).is_err());
        assert!(ma_playback(false, Some("ipfs://bafy".into())).is_err());
        assert!(ma_playback(false, Some("magnet:?xt=urn:btih:abc".into())).is_err());
    }

    #[test]
    fn stream_url_uses_scoped_key_and_youtube_flag() {
        let url = build_stream_url("http://192.168.1.20:8040", 42, "rss key", 7, false);
        assert_eq!(
            url,
            "http://192.168.1.20:8040/api/data/stream/42?api_key=rss%20key&user_id=7"
        );
        let yt = build_stream_url("http://192.168.1.20:8040/", 3, "k", 1, true);
        assert!(yt.ends_with("&type=youtube"));
        let parsed = parse_stream_url(&yt).unwrap();
        assert_eq!(parsed.episode_id, 3);
        assert_eq!(parsed.user_id, Some(1));
        assert!(parsed.is_youtube);
    }

    #[test]
    fn device_id_roundtrips_player_ids_that_contain_colons() {
        let id = device_id_for(4, "b8:27:eb:8a:b8:8e");
        assert_eq!(id, "ma:4:b8:27:eb:8a:b8:8e");
        assert_eq!(
            player_id_from_device(4, &id).as_deref(),
            Some("b8:27:eb:8a:b8:8e")
        );
        assert!(player_id_from_device(5, &id).is_none());
        assert_eq!(canonical_player_id(4, &id), "b8:27:eb:8a:b8:8e");
    }

    #[test]
    fn version_check_accepts_2_10_and_refuses_ancient() {
        let ok = assess_version("2.10.3", Some(80));
        assert!(ok.compatible);
        assert!(ok.warning.is_none());
        assert!(!assess_version("1.9.0", None).compatible);
        assert!(!assess_version("2.5.1", Some(10)).compatible);
        let older = assess_version("2.8.0", None);
        assert!(older.compatible && older.warning.is_some());
        let future = assess_version("3.0.0", Some(200));
        assert!(future.compatible && future.warning.is_some());
        let unparsed = assess_version("not-a-version", None);
        assert!(unparsed.compatible && unparsed.warning.is_some());
        let old_schema = assess_version("2.10.0", Some(5));
        assert!(old_schema.compatible && old_schema.warning.is_some());
    }

    #[test]
    fn translate_transport_volume_and_grouping() {
        let pause = translate_action("kitchen", "kitchen", "pause", &json!({})).unwrap();
        assert_eq!(pause[0].command, "players/cmd/pause");
        assert_eq!(pause[0].args["player_id"], "kitchen");

        let seek =
            translate_action("kitchen", "q1", "seek", &json!({"position_sec": 12.5})).unwrap();
        assert_eq!(seek[0].command, "player_queues/seek");
        assert_eq!(seek[0].args["queue_id"], "q1");
        assert_eq!(seek[0].args["position"], 12.5);

        let back = translate_action("kitchen", "q1", "skip_back", &json!({})).unwrap();
        assert_eq!(back[0].args["seconds"], -15.0);

        let vol =
            translate_action("kitchen", "q1", "volume_set", &json!({"volume_level": 140})).unwrap();
        assert_eq!(vol[0].command, "players/cmd/volume_set");
        assert_eq!(vol[0].args["volume_level"], 100.0);

        let mute =
            translate_action("kitchen", "q1", "volume_mute", &json!({"muted": true})).unwrap();
        assert_eq!(mute[0].args["muted"], true);

        let group =
            translate_action("bed", "bed", "group", &json!({"target_player": "kitchen"})).unwrap();
        assert_eq!(group[0].command, "players/cmd/group");
        assert_eq!(group[0].args["target_player"], "kitchen");

        let many = translate_action(
            "kitchen",
            "kitchen",
            "group_many",
            &json!({"player_ids": ["bed", "patio"]}),
        )
        .unwrap();
        assert_eq!(many[0].command, "players/cmd/group_many");
        assert_eq!(many[0].args["target_player"], "kitchen");
        assert_eq!(many[0].args["player_ids"][1], "patio");

        let members = translate_action(
            "grp",
            "grp",
            "set_members",
            &json!({"player_ids_to_add": ["a"], "player_ids_to_remove": []}),
        )
        .unwrap();
        assert_eq!(members[0].command, "players/cmd/set_members");

        let speed = translate_action("kitchen", "q1", "set_speed", &json!({"speed": 1.5})).unwrap();
        assert_eq!(speed[0].command, "player_queues/set_playback_speed");
        assert!(translate_action("kitchen", "q1", "nope", &json!({})).is_err());
    }

    #[test]
    fn play_media_seeks_and_sets_speed_when_needed() {
        let cmds = play_media_commands(
            "q",
            "http://h/api/data/stream/1?api_key=k&user_id=2",
            30.0,
            1.25,
        );
        assert_eq!(cmds.len(), 3);
        assert_eq!(cmds[0].command, "player_queues/play_media");
        assert_eq!(
            cmds[0].args["media"],
            "http://h/api/data/stream/1?api_key=k&user_id=2"
        );
        assert_eq!(cmds[0].args["option"], "replace");
        assert_eq!(cmds[1].command, "player_queues/seek");
        assert_eq!(cmds[2].args["speed"], 1.25);
        let plain = play_media_commands("q", "http://h/a", 0.0, 1.0);
        assert_eq!(plain.len(), 1);
    }

    #[test]
    fn parses_results_errors_and_events() {
        match parse_inbound(r#"{"message_id":"7","result":{"authenticated":true}}"#) {
            Inbound::Result { message_id, result } => {
                assert_eq!(message_id, "7");
                assert_eq!(result["authenticated"], true);
            }
            other => panic!("expected result, got {other:?}"),
        }
        match parse_inbound(r#"{"message_id":"7","error_code":4,"details":"nope"}"#) {
            Inbound::Error {
                details,
                error_code,
                ..
            } => {
                assert_eq!(details, "nope");
                assert_eq!(error_code, 4);
            }
            other => panic!("expected error, got {other:?}"),
        }
        match parse_inbound(
            r#"{"event":"queue_time_updated","object_id":"k","data":{"elapsed_time":3}}"#,
        ) {
            Inbound::Event {
                event,
                object_id,
                data,
            } => {
                assert_eq!(event, "queue_time_updated");
                assert_eq!(object_id.as_deref(), Some("k"));
                assert_eq!(data["elapsed_time"], 3);
            }
            other => panic!("expected event, got {other:?}"),
        }
    }

    #[test]
    fn parses_players_groups_and_queue_progress() {
        let players = players_from_result(&json!([
            {
                "player_id": "kitchen",
                "name": "Kitchen",
                "available": true,
                "state": "playing",
                "volume_level": 35,
                "volume_muted": false,
                "type": "player",
                "group_childs": [],
                "active_source": "kitchen"
            },
            {
                "player_id": "downstairs",
                "name": "Downstairs",
                "type": "group",
                "group_childs": ["kitchen", {"player_id": "patio"}],
                "can_group_with": ["bed"]
            }
        ]));
        assert_eq!(players.len(), 2);
        assert_eq!(players[0].kind, MaPlayerKind::Player);
        assert_eq!(players[0].volume_level, Some(35.0));
        assert_eq!(players[1].kind, MaPlayerKind::Group);
        assert_eq!(
            players[1].group_members,
            vec!["kitchen".to_string(), "patio".to_string()]
        );

        let update = progress_from_payload(&json!({
            "queue_id": "kitchen",
            "state": "playing",
            "elapsed_time": 42.5,
            "current_item": {
                "name": "Show notes",
                "duration": 1800,
                "media_item": {
                    "uri": "http://192.168.1.20:8040/api/data/stream/15?api_key=abc&user_id=3"
                }
            }
        }));
        assert_eq!(update.queue_id.as_deref(), Some("kitchen"));
        assert_eq!(update.position_sec, Some(42.5));
        assert_eq!(update.duration_sec, Some(1800.0));
        assert_eq!(update.playing, Some(true));
        let stream = update.stream.unwrap();
        assert_eq!(stream.episode_id, 15);
        assert_eq!(stream.user_id, Some(3));
        assert!(!stream.is_youtube);

        let tick = progress_from_payload(&json!({"queue_id": "kitchen", "elapsed_time": 43.0}));
        assert!(tick.stream.is_none());
        assert_eq!(tick.position_sec, Some(43.0));
    }

    #[test]
    fn progress_writes_are_throttled_until_pause_seek_or_interval() {
        let first = ProgressSample {
            episode_id: 1,
            position_sec: 10.0,
            playing: true,
        };
        assert!(should_write_progress(None, &first, Duration::from_secs(0)));
        let later = ProgressSample {
            episode_id: 1,
            position_sec: 12.0,
            playing: true,
        };
        assert!(!should_write_progress(
            Some(&first),
            &later,
            Duration::from_secs(3)
        ));
        assert!(should_write_progress(
            Some(&first),
            &later,
            Duration::from_secs(10)
        ));
        let paused = ProgressSample {
            episode_id: 1,
            position_sec: 12.0,
            playing: false,
        };
        assert!(should_write_progress(
            Some(&later),
            &paused,
            Duration::from_secs(1)
        ));
        let seek = ProgressSample {
            episode_id: 1,
            position_sec: 80.0,
            playing: true,
        };
        assert!(should_write_progress(
            Some(&later),
            &seek,
            Duration::from_secs(1)
        ));
    }

    #[test]
    fn relay_command_frame_parses() {
        let (action, args) = parse_relay_command(
            r#"{"type":"command","from_device_id":"web","action":"pause","args":{}}"#,
        )
        .unwrap();
        assert_eq!(action, "pause");
        assert!(args.as_object().unwrap().is_empty());
        assert!(parse_relay_command(r#"{"type":"devices","devices":[]}"#).is_none());
    }

    async fn recv_json(socket: &mut WebSocketStream<DuplexStream>) -> Value {
        loop {
            match socket.next().await.unwrap().unwrap() {
                Message::Text(t) => return serde_json::from_str(t.as_ref()).unwrap(),
                Message::Ping(p) => {
                    socket.send(Message::Pong(p)).await.unwrap();
                }
                other => panic!("unexpected frame {other:?}"),
            }
        }
    }

    async fn send_json(socket: &mut WebSocketStream<DuplexStream>, v: Value) {
        socket
            .send(Message::Text(v.to_string().into()))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn session_auths_lists_players_controls_and_reports_progress() {
        let (client_io, server_io) = tokio::io::duplex(64 * 1024);
        let client = WebSocketStream::from_raw_socket(client_io, Role::Client, None).await;
        let mut server = WebSocketStream::from_raw_socket(server_io, Role::Server, None).await;

        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (ev_tx, mut ev_rx) = mpsc::unbounded_channel();
        let drive =
            tokio::spawn(async move { drive_socket(client, "secret-token", cmd_rx, ev_tx).await });

        let auth = recv_json(&mut server).await;
        assert_eq!(auth["command"], "auth");
        assert_eq!(auth["args"]["token"], "secret-token");
        let auth_id = auth["message_id"].as_str().unwrap().to_string();
        send_json(
            &mut server,
            json!({"message_id": auth_id, "result": {"authenticated": true}}),
        )
        .await;

        let info = recv_json(&mut server).await;
        assert_eq!(info["command"], "info");
        let info_id = info["message_id"].as_str().unwrap().to_string();
        send_json(
            &mut server,
            json!({
                "message_id": info_id,
                "result": {"server_version": "2.10.3", "schema_version": 80, "server_id": "srv"}
            }),
        )
        .await;

        let players = recv_json(&mut server).await;
        assert_eq!(players["command"], "players/all");
        let players_id = players["message_id"].as_str().unwrap().to_string();
        send_json(
            &mut server,
            json!({
                "message_id": players_id,
                "result": [{
                    "player_id": "kitchen",
                    "name": "Kitchen",
                    "available": true,
                    "state": "idle",
                    "type": "player",
                    "volume_level": 20
                }]
            }),
        )
        .await;

        let mut saw_server = false;
        let mut saw_players = false;
        while let Some(note) = ev_rx.recv().await {
            match note {
                SessionNote::Server(info) => {
                    assert_eq!(info.server_version, "2.10.3");
                    assert!(info.assessment.compatible);
                    saw_server = true;
                }
                SessionNote::Players(list) => {
                    assert_eq!(list.len(), 1);
                    assert_eq!(list[0].player_id, "kitchen");
                    saw_players = true;
                    break;
                }
                SessionNote::Event { .. } => {}
            }
        }
        assert!(saw_server && saw_players);

        cmd_tx
            .send(Outbound {
                command: "player_queues/play_media".into(),
                args: json!({
                    "queue_id": "kitchen",
                    "media": "http://192.168.1.20:8040/api/data/stream/15?api_key=abc&user_id=3",
                    "option": "replace"
                }),
                reply: None,
            })
            .unwrap();

        let play = recv_json(&mut server).await;
        assert_eq!(play["command"], "player_queues/play_media");
        assert_eq!(
            play["args"]["media"],
            "http://192.168.1.20:8040/api/data/stream/15?api_key=abc&user_id=3"
        );
        let play_id = play["message_id"].as_str().unwrap().to_string();
        send_json(&mut server, json!({"message_id": play_id, "result": true})).await;
        send_json(
            &mut server,
            json!({
                "event": "queue_updated",
                "object_id": "kitchen",
                "data": {
                    "queue_id": "kitchen",
                    "state": "playing",
                    "elapsed_time": 1.5,
                    "current_item": {
                        "duration": 100,
                        "media_item": {
                            "uri": "http://192.168.1.20:8040/api/data/stream/15?api_key=abc&user_id=3"
                        }
                    }
                }
            }),
        )
        .await;

        let mut progress = None;
        while let Some(note) = ev_rx.recv().await {
            if let SessionNote::Event { event, data, .. } = note {
                assert_eq!(event, "queue_updated");
                progress = Some(progress_from_payload(&data));
                break;
            }
        }
        let progress = progress.expect("queue event");
        assert_eq!(progress.stream.unwrap().episode_id, 15);
        assert_eq!(progress.position_sec, Some(1.5));
        assert_eq!(progress.playing, Some(true));

        drive.abort();
    }

    #[tokio::test]
    async fn auth_failure_ends_the_session() {
        let (client_io, server_io) = tokio::io::duplex(8 * 1024);
        let client = WebSocketStream::from_raw_socket(client_io, Role::Client, None).await;
        let mut server = WebSocketStream::from_raw_socket(server_io, Role::Server, None).await;
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<Outbound>();
        let (ev_tx, _ev_rx) = mpsc::unbounded_channel();
        let drive = tokio::spawn(async move { drive_socket(client, "nope", cmd_rx, ev_tx).await });
        let auth = recv_json(&mut server).await;
        let id = auth["message_id"].as_str().unwrap().to_string();
        send_json(
            &mut server,
            json!({"message_id": id, "error_code": 1, "details": "Invalid or expired token"}),
        )
        .await;
        let err = drive.await.unwrap().unwrap_err();
        assert!(err.contains("Invalid or expired token"), "{err}");
        drop(cmd_tx);
    }
}
