// Music Assistant settings, discovery, and control for the web client.
//
// The HTTP routes are the contract in docs/music-assistant.md. Playback state
// for players that are online also arrives on the now-playing relay
// (`device_type` `music_assistant`). This module owns the request types, the
// small pieces of UI logic that need tests (error copy, grouping, which device
// the now-playing bar follows), and the yewdux store the settings page and the
// speaker picker share.

use anyhow::Error;
use gloo_net::http::Request;
use serde::Deserialize;
use serde_json::{json, Value};
use yewdux::prelude::*;

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct MaSettings {
    pub enabled: bool,
    #[serde(default)]
    pub server_url: Option<String>,
    #[serde(default)]
    pub external_base_url: Option<String>,
    #[serde(default)]
    pub has_token: bool,
    #[serde(default)]
    pub connected: bool,
    #[serde(default)]
    pub server_version: Option<String>,
    #[serde(default)]
    pub schema_version: Option<i64>,
    #[serde(default)]
    pub warning: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct MaTestReport {
    pub ok: bool,
    pub detail: String,
    #[serde(default)]
    pub server_version: Option<String>,
    #[serde(default)]
    pub schema_version: Option<i64>,
    #[serde(default)]
    pub player_count: i64,
    #[serde(default)]
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct MaPlayer {
    pub player_id: String,
    pub device_id: String,
    pub name: String,
    /// `player` or `group`.
    pub kind: String,
    #[serde(default)]
    pub available: bool,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub volume_level: Option<f64>,
    #[serde(default)]
    pub volume_muted: bool,
    #[serde(default)]
    pub group_members: Vec<String>,
    #[serde(default)]
    pub synced_to: Option<String>,
    #[serde(default)]
    pub can_group_with: Vec<String>,
    #[serde(default)]
    pub queue_id: String,
}

impl MaPlayer {
    pub fn is_group(&self) -> bool {
        self.kind.eq_ignore_ascii_case("group") || !self.group_members.is_empty()
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct MaPlayersResponse {
    pub enabled: bool,
    pub connected: bool,
    pub players: Vec<MaPlayer>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct MaCommandResponse {
    pub ok: bool,
    pub detail: String,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PendingEpisode {
    pub episode_id: i32,
    pub is_youtube: bool,
}

/// Shared by the settings page, the speaker picker, and the now-playing bar.
/// Speaker controls read `enabled` and stay hidden until it is true.
#[derive(Default, Clone, PartialEq, Store)]
pub struct MusicAssistantState {
    pub loaded: bool,
    pub enabled: bool,
    pub connected: bool,
    pub server_version: Option<String>,
    pub warning: Option<String>,
    pub last_error: Option<String>,
    pub players: Vec<MaPlayer>,
    pub picker_open: bool,
    /// Set when an episode menu asks the picker to start that episode.
    pub pending_episode: Option<PendingEpisode>,
    /// Speaker the user last sent audio to. The now-playing bar prefers it.
    pub focus_device_id: Option<String>,
}

pub fn apply_settings(view: &MaSettings) {
    Dispatch::<MusicAssistantState>::global().reduce_mut(|s| {
        s.loaded = true;
        s.enabled = view.enabled;
        s.connected = view.connected;
        s.server_version = view.server_version.clone();
        s.warning = view.warning.clone();
        s.last_error = view.last_error.clone();
        if !view.enabled {
            s.players.clear();
            s.picker_open = false;
        }
    });
}

pub fn apply_players(list: MaPlayersResponse) {
    Dispatch::<MusicAssistantState>::global().reduce_mut(|s| {
        s.loaded = true;
        s.enabled = list.enabled;
        s.connected = list.connected;
        s.players = if list.enabled { list.players } else { Vec::new() };
        if !list.enabled {
            s.picker_open = false;
        }
    });
}

/// Open the speaker picker for one episode (episode context menu).
pub fn open_speaker_picker(episode_id: i32, is_youtube: bool) {
    Dispatch::<MusicAssistantState>::global().reduce_mut(|s| {
        s.pending_episode = Some(PendingEpisode {
            episode_id,
            is_youtube,
        });
        s.picker_open = true;
    });
}

pub fn focus_player(device_id: &str) {
    Dispatch::<MusicAssistantState>::global().reduce_mut(|s| {
        s.focus_device_id = Some(device_id.to_string());
    });
}

// ============================ Errors =====================================

/// Why a settings or test call failed, mapped to a short explanation in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaProblem {
    /// External base URL is localhost, loopback, or link-local.
    LocalhostRejected,
    /// The Music Assistant WebSocket could not be opened.
    Unreachable,
    /// Playback can start and then stop because MA cannot fetch the stream.
    StreamHint,
    NotConfigured,
    AuthFailed,
    Other,
}

/// Result of a Test click: the probe outcome plus which explanations to show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestExplanation {
    pub ok: bool,
    pub problems: Vec<MaProblem>,
    /// True when the form should repeat that MA must reach the stream URL.
    pub show_stream_hint: bool,
    pub detail: String,
}

/// Pull `message` out of an `{error, message, status_code}` body. Other text
/// is returned unchanged.
pub fn message_from_error_body(body: &str) -> String {
    let trimmed = body.trim();
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        if let Some(message) = value.get("message").and_then(Value::as_str) {
            let message = message.trim();
            if !message.is_empty() {
                return message.to_string();
            }
        }
    }
    trimmed.to_string()
}

/// Host of an http(s) URL, without port or brackets. Empty when it cannot be split.
pub fn external_host(raw: &str) -> String {
    let trimmed = raw.trim();
    let after_scheme = trimmed.split_once("://").map(|(_, rest)| rest).unwrap_or(trimmed);
    let hostport = after_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(after_scheme);
    let host = if let Some(rest) = hostport.strip_prefix('[') {
        rest.split(']').next().unwrap_or(rest)
    } else {
        hostport.split(':').next().unwrap_or(hostport)
    };
    host.trim().to_ascii_lowercase()
}

/// Client-side mirror of the server's external-base rejection, so the form can
/// explain the failure before the round trip. The server stays authoritative.
pub fn external_base_is_rejected(raw: &str) -> bool {
    let host = external_host(raw);
    if host.is_empty() {
        return false;
    }
    if host == "localhost"
        || host == "localhost.localdomain"
        || host.ends_with(".localhost")
        || host == "metadata.google.internal"
        || host.ends_with(".metadata.google.internal")
    {
        return true;
    }
    if host == "127.0.0.1" || host == "::1" || host == "0.0.0.0" || host == "::" {
        return true;
    }
    if host.starts_with("169.254.") {
        return true;
    }
    // fe80::/10, written without brackets after `external_host` strips them.
    let v6 = host.split('%').next().unwrap_or(&host);
    v6.starts_with("fe80:")
}

pub fn classify_ma_text(text: &str) -> MaProblem {
    let lower = text.to_ascii_lowercase();
    if lower.contains("localhost")
        || lower.contains("loopback")
        || lower.contains("link-local")
        || lower.contains("must be a lan")
        || lower.contains("music assistant can reach")
    {
        return MaProblem::LocalhostRejected;
    }
    if lower.contains("could not connect")
        || lower.contains("timed out")
        || lower.contains("connection refused")
        || lower.contains("unreachable")
        || lower.contains("os error")
        || lower.contains("dns error")
        || lower.contains("network is unreachable")
    {
        return MaProblem::Unreachable;
    }
    if lower.contains("stream")
        && (lower.contains("reach") || lower.contains("fetch") || lower.contains("external"))
    {
        return MaProblem::StreamHint;
    }
    if lower.contains("not configured") || lower.contains("is required") {
        return MaProblem::NotConfigured;
    }
    if lower.contains("invalid or expired token")
        || lower.contains("authentication")
        || lower.contains("unauthorized")
        || (lower.contains("token") && (lower.contains("invalid") || lower.contains("expired")))
    {
        return MaProblem::AuthFailed;
    }
    MaProblem::Other
}

fn push_unique(problems: &mut Vec<MaProblem>, problem: MaProblem) {
    if !problems.contains(&problem) {
        problems.push(problem);
    }
}

/// Combine the typed external base URL with the probe (or the HTTP error text).
pub fn explain_test(external_base: &str, probe: Result<&MaTestReport, &str>) -> TestExplanation {
    let mut problems = Vec::new();
    let mut details: Vec<String> = Vec::new();
    if external_base_is_rejected(external_base) {
        push_unique(&mut problems, MaProblem::LocalhostRejected);
        push_unique(&mut problems, MaProblem::StreamHint);
    }
    match probe {
        Ok(report) => {
            if let Some(warning) = report.warning.as_deref().filter(|w| !w.is_empty()) {
                details.push(warning.to_string());
            }
            if !report.detail.is_empty() {
                details.push(report.detail.clone());
            }
            if !report.ok {
                push_unique(&mut problems, classify_ma_text(&report.detail));
            }
            let blocking = problems.iter().any(|p| *p != MaProblem::StreamHint);
            TestExplanation {
                ok: report.ok && !blocking,
                problems,
                show_stream_hint: true,
                detail: details.join(" — "),
            }
        }
        Err(err) => {
            let message = message_from_error_body(err);
            let problem = classify_ma_text(&message);
            push_unique(&mut problems, problem);
            if problem == MaProblem::LocalhostRejected || problem == MaProblem::StreamHint {
                push_unique(&mut problems, MaProblem::StreamHint);
            }
            if !message.is_empty() {
                details.push(message);
            }
            TestExplanation {
                ok: false,
                show_stream_hint: problems.contains(&MaProblem::StreamHint)
                    || problems.contains(&MaProblem::LocalhostRejected),
                problems,
                detail: details.join(" — "),
            }
        }
    }
}

// ============================ Grouping ===================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupPlan {
    /// `players/cmd/group_many` via the relay or the command route.
    GroupMany {
        target_player: String,
        player_ids: Vec<String>,
    },
    /// Adjust one existing group or sync leader.
    SetMembers {
        player_id: String,
        add: Vec<String>,
        remove: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupPlanError {
    NeedTwo,
    MultipleGroups,
    Unchanged,
}

impl GroupPlan {
    pub fn action(&self) -> &'static str {
        match self {
            GroupPlan::GroupMany { .. } => "group_many",
            GroupPlan::SetMembers { .. } => "set_members",
        }
    }

    /// Player id the command is addressed to.
    pub fn player_id(&self) -> &str {
        match self {
            GroupPlan::GroupMany { target_player, .. } => target_player,
            GroupPlan::SetMembers { player_id, .. } => player_id,
        }
    }

    pub fn args(&self) -> Value {
        match self {
            GroupPlan::GroupMany {
                target_player,
                player_ids,
            } => json!({
                "player_ids": player_ids,
                "target_player": target_player,
            }),
            GroupPlan::SetMembers { add, remove, .. } => json!({
                "player_ids_to_add": add,
                "player_ids_to_remove": remove,
            }),
        }
    }
}

/// Turn a checkbox selection into one grouping command.
///
/// One selected group (or sync leader that already has members) is edited with
/// `set_members`. Several plain players become `group_many`, led by the first
/// selected id.
pub fn plan_group(players: &[MaPlayer], selected: &[String]) -> Result<GroupPlan, GroupPlanError> {
    if selected.len() < 2 {
        return Err(GroupPlanError::NeedTwo);
    }
    let groups: Vec<&MaPlayer> = players
        .iter()
        .filter(|p| selected.iter().any(|id| id == &p.player_id) && p.is_group())
        .collect();
    if groups.len() > 1 {
        return Err(GroupPlanError::MultipleGroups);
    }
    if groups.len() == 1 {
        let group = groups[0];
        let desired: Vec<&String> = selected.iter().filter(|id| *id != &group.player_id).collect();
        let add: Vec<String> = desired
            .iter()
            .filter(|id| !group.group_members.iter().any(|m| m == **id))
            .map(|id| (*id).clone())
            .collect();
        let remove: Vec<String> = group
            .group_members
            .iter()
            .filter(|id| !desired.iter().any(|d| *d == *id))
            .cloned()
            .collect();
        if add.is_empty() && remove.is_empty() {
            return Err(GroupPlanError::Unchanged);
        }
        return Ok(GroupPlan::SetMembers {
            player_id: group.player_id.clone(),
            add,
            remove,
        });
    }
    Ok(GroupPlan::GroupMany {
        target_player: selected[0].clone(),
        player_ids: selected.to_vec(),
    })
}

/// A now-playing row the bar can follow. Built from relay snapshots.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaybackCandidate {
    pub device_id: String,
    pub device_type: String,
    pub episode_id: i32,
    pub playing: bool,
    pub updated_at: i64,
}

pub fn is_music_assistant(device_type: &str) -> bool {
    device_type.eq_ignore_ascii_case("music_assistant")
}

/// Music Assistant players belong on the speaker panel, not in the generic
/// "other devices" list. Every other now-playing device stays there.
pub fn include_in_other_devices(device_type: &str) -> bool {
    !is_music_assistant(device_type)
}

/// `Some(true)` shows pause, `Some(false)` shows play. `None` hides transport.
///
/// Playing and buffering show pause. Paused shows play. Idle, unknown, off,
/// and a missing state still show play while an episode is loaded or the relay
/// says the speaker is playing. Sonos reports idle for a Music Assistant pause
/// and for a sync member whose coordinator is playing; hiding the buttons there
/// leaves nothing to press.
pub fn transport_shown(player_state: &str, episode_loaded: bool, relay_playing: bool) -> Option<bool> {
    let state = player_state.trim();
    let playing = relay_playing
        || state.eq_ignore_ascii_case("playing")
        || state.eq_ignore_ascii_case("buffering");
    if playing {
        return Some(true);
    }
    if state.eq_ignore_ascii_case("paused") || episode_loaded {
        return Some(false);
    }
    None
}

/// Playback state the transport buttons should follow. A synced speaker's own
/// state is often idle while the leader is playing.
pub fn playback_state_for<'a>(player: &'a MaPlayer, players: &'a [MaPlayer]) -> &'a str {
    if let Some(id) = player
        .synced_to
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
    {
        if let Some(leader) = players.iter().find(|p| p.player_id == id) {
            if !leader.state.trim().is_empty() {
                return leader.state.as_str();
            }
        }
    }
    player.state.as_str()
}

/// Which MA device the now-playing bar should show.
/// Prefers the speaker the user last targeted, then one that is playing,
/// then the freshest snapshot that has an episode loaded.
/// The now-playing bar follows Music Assistant when the bridge is on, this
/// device is not the one playing, and some MA player has an episode loaded.
pub fn show_ma_now_playing(
    enabled: bool,
    local_playing: bool,
    devices: &[PlaybackCandidate],
    focus_device_id: Option<&str>,
) -> bool {
    enabled && !local_playing && choose_playback(devices, focus_device_id).is_some()
}

pub fn choose_playback<'a>(
    devices: &'a [PlaybackCandidate],
    focus_device_id: Option<&str>,
) -> Option<&'a PlaybackCandidate> {
    let mut matched: Vec<&PlaybackCandidate> = devices
        .iter()
        .filter(|d| is_music_assistant(&d.device_type) && d.episode_id != 0)
        .collect();
    if matched.is_empty() {
        return None;
    }
    if let Some(id) = focus_device_id {
        if let Some(found) = matched.iter().copied().find(|d| d.device_id == id) {
            return Some(found);
        }
    }
    if let Some(playing) = matched.iter().copied().find(|d| d.playing) {
        return Some(playing);
    }
    matched.sort_by_key(|d| d.updated_at);
    matched.pop()
}

/// Advance a relay position between snapshots. Stops extrapolating once the
/// snapshot is older than 30 seconds so a dropped socket does not run away.
pub fn displayed_position(
    position_sec: f64,
    playing: bool,
    speed: f64,
    updated_at: i64,
    now: i64,
) -> f64 {
    let base = if position_sec.is_finite() {
        position_sec.max(0.0)
    } else {
        0.0
    };
    if !playing || updated_at <= 0 {
        return base;
    }
    let elapsed = (now - updated_at) as f64;
    if elapsed < 0.0 || elapsed > 30.0 {
        return base;
    }
    let rate = if speed.is_finite() && speed > 0.0 {
        speed
    } else {
        1.0
    };
    base + elapsed * rate
}

pub fn format_clock(secs: f64) -> String {
    if !secs.is_finite() || secs < 0.0 {
        return "00:00".to_string();
    }
    let total = secs as i64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

pub fn player_name<'a>(players: &'a [MaPlayer], player_id: &'a str) -> &'a str {
    players
        .iter()
        .find(|p| p.player_id == player_id)
        .map(|p| p.name.as_str())
        .filter(|n| !n.is_empty())
        .unwrap_or(player_id)
}

// ============================ HTTP =======================================

fn nonempty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

async fn error_from_response(response: gloo_net::http::Response) -> Error {
    let status = response.status();
    let body = response
        .text()
        .await
        .unwrap_or_else(|_| String::from("Failed to read error message"));
    let message = message_from_error_body(&body);
    Error::msg(format!("{status}: {message}"))
}

pub async fn get_settings(
    server_name: &str,
    api_key: &str,
    user_id: i32,
) -> Result<MaSettings, Error> {
    let url = format!("{server_name}/api/data/music_assistant/settings?user_id={user_id}");
    let response = Request::get(&url).header("Api-Key", api_key).send().await?;
    if response.ok() {
        Ok(response.json().await?)
    } else {
        Err(error_from_response(response).await)
    }
}

pub async fn save_settings(
    server_name: &str,
    api_key: &str,
    user_id: i32,
    enabled: bool,
    server_url: &str,
    external_base_url: &str,
    token: &str,
    clear_token: bool,
) -> Result<MaSettings, Error> {
    let url = format!("{server_name}/api/data/music_assistant/settings");
    let body = json!({
        "user_id": user_id,
        "enabled": enabled,
        "server_url": nonempty(server_url),
        "external_base_url": nonempty(external_base_url),
        "token": nonempty(token),
        "clear_token": clear_token,
    });
    let response = Request::put(&url)
        .header("Api-Key", api_key)
        .header("Content-Type", "application/json")
        .body(body.to_string())?
        .send()
        .await?;
    if response.ok() {
        Ok(response.json().await?)
    } else {
        Err(error_from_response(response).await)
    }
}

pub async fn test_connection(
    server_name: &str,
    api_key: &str,
    user_id: i32,
    server_url: &str,
    external_base_url: &str,
    token: &str,
) -> Result<MaTestReport, Error> {
    let url = format!("{server_name}/api/data/music_assistant/test");
    let body = json!({
        "user_id": user_id,
        "server_url": nonempty(server_url),
        "external_base_url": nonempty(external_base_url),
        "token": nonempty(token),
    });
    let response = Request::post(&url)
        .header("Api-Key", api_key)
        .header("Content-Type", "application/json")
        .body(body.to_string())?
        .send()
        .await?;
    if response.ok() {
        Ok(response.json().await?)
    } else {
        Err(error_from_response(response).await)
    }
}

pub async fn list_players(
    server_name: &str,
    api_key: &str,
    user_id: i32,
) -> Result<MaPlayersResponse, Error> {
    let url = format!("{server_name}/api/data/music_assistant/players?user_id={user_id}");
    let response = Request::get(&url).header("Api-Key", api_key).send().await?;
    if response.ok() {
        Ok(response.json().await?)
    } else {
        Err(error_from_response(response).await)
    }
}

pub async fn send_player_command(
    server_name: &str,
    api_key: &str,
    user_id: i32,
    player_id: &str,
    action: &str,
    args: Value,
) -> Result<MaCommandResponse, Error> {
    let url = format!("{server_name}/api/data/music_assistant/command");
    let body = json!({
        "user_id": user_id,
        "player_id": player_id,
        "action": action,
        "args": args,
    });
    let response = Request::post(&url)
        .header("Api-Key", api_key)
        .header("Content-Type", "application/json")
        .body(body.to_string())?
        .send()
        .await?;
    if response.ok() {
        Ok(response.json().await?)
    } else {
        Err(error_from_response(response).await)
    }
}

/// Reload settings, and players when the bridge is on. Safe to call with no
/// session: it does nothing.
pub fn refresh_music_assistant() {
    let app = Dispatch::<crate::components::context::AppState>::global().get();
    let Some(auth) = app.auth_details.as_ref() else {
        return;
    };
    let Some(api_key) = auth.api_key.clone() else {
        return;
    };
    let Some(user) = app.user_details.as_ref() else {
        return;
    };
    let server = auth.server_name.clone();
    let user_id = user.UserID;
    wasm_bindgen_futures::spawn_local(async move {
        match get_settings(&server, &api_key, user_id).await {
            Ok(view) => {
                let enabled = view.enabled;
                apply_settings(&view);
                if enabled {
                    if let Ok(list) = list_players(&server, &api_key, user_id).await {
                        apply_players(list);
                    }
                }
            }
            Err(err) => {
                web_sys::console::warn_1(&format!("music assistant settings: {err}").into());
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::wasm_bindgen_test;

    fn player(id: &str, kind: &str, members: &[&str]) -> MaPlayer {
        MaPlayer {
            player_id: id.into(),
            device_id: format!("ma:1:{id}"),
            name: id.into(),
            kind: kind.into(),
            available: true,
            state: "idle".into(),
            volume_level: Some(20.0),
            volume_muted: false,
            group_members: members.iter().map(|m| (*m).to_string()).collect(),
            synced_to: None,
            can_group_with: vec!["living".into()],
            queue_id: id.into(),
        }
    }

    fn candidate(id: &str, episode: i32, playing: bool, updated: i64) -> PlaybackCandidate {
        PlaybackCandidate {
            device_id: id.into(),
            device_type: "music_assistant".into(),
            episode_id: episode,
            playing,
            updated_at: updated,
        }
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn error_body_uses_message_and_leaves_plain_text() {
        let body = r#"{"error":"Bad request","message":"Bad request: external base URL must be a LAN or public host Music Assistant can reach, not localhost","status_code":400}"#;
        assert!(message_from_error_body(body).contains("not localhost"));
        assert_eq!(message_from_error_body("timed out connecting"), "timed out connecting");
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn localhost_external_urls_are_rejected() {
        assert!(external_base_is_rejected("http://localhost:8040"));
        assert!(external_base_is_rejected("http://127.0.0.1:8040"));
        assert!(external_base_is_rejected("http://[::1]:8040"));
        assert!(external_base_is_rejected("http://app.localhost"));
        assert!(external_base_is_rejected("http://169.254.1.1"));
        assert!(external_base_is_rejected("http://[fe80::1]/"));
        assert!(!external_base_is_rejected("http://192.168.1.10:8040"));
        assert!(!external_base_is_rejected("https://pods.example.com"));
        assert!(!external_base_is_rejected(""));
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn classify_known_failures() {
        assert_eq!(
            classify_ma_text("external base URL must be a LAN or public host Music Assistant can reach, not localhost"),
            MaProblem::LocalhostRejected
        );
        assert_eq!(
            classify_ma_text("could not connect: connection refused"),
            MaProblem::Unreachable
        );
        assert_eq!(
            classify_ma_text("timed out connecting to Music Assistant"),
            MaProblem::Unreachable
        );
        assert_eq!(
            classify_ma_text("Music Assistant cannot fetch the stream URL"),
            MaProblem::StreamHint
        );
        assert_eq!(
            classify_ma_text("a Music Assistant token is required to enable the connection"),
            MaProblem::NotConfigured
        );
        assert_eq!(
            classify_ma_text("Invalid or expired token"),
            MaProblem::AuthFailed
        );
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn explain_test_surfaces_localhost_and_unreachable() {
        let probe = MaTestReport {
            ok: false,
            detail: "could not connect: connection refused".into(),
            server_version: None,
            schema_version: None,
            player_count: 0,
            warning: None,
        };
        let explained = explain_test("http://localhost:8040", Ok(&probe));
        assert!(!explained.ok);
        assert!(explained.problems.contains(&MaProblem::LocalhostRejected));
        assert!(explained.problems.contains(&MaProblem::Unreachable));
        assert!(explained.show_stream_hint);

        let server_err = r#"{"message":"Bad request: external base URL must be a LAN or public host Music Assistant can reach, not localhost"}"#;
        let explained = explain_test("http://192.168.1.10:8040", Err(server_err));
        assert!(explained.problems.contains(&MaProblem::LocalhostRejected));
        assert!(explained.show_stream_hint);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn explain_test_success_keeps_the_stream_hint() {
        let probe = MaTestReport {
            ok: true,
            detail: "connected, 2 player(s)".into(),
            server_version: Some("2.10.3".into()),
            schema_version: Some(80),
            player_count: 2,
            warning: None,
        };
        let explained = explain_test("http://192.168.1.10:8040", Ok(&probe));
        assert!(explained.ok);
        assert!(explained.problems.is_empty());
        assert!(explained.show_stream_hint);
        assert!(explained.detail.contains("2 player"));
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn plan_group_creates_and_edits() {
        let players = vec![
            player("kitchen", "player", &[]),
            player("living", "player", &[]),
            player("den", "player", &[]),
        ];
        let plan = plan_group(
            &players,
            &["kitchen".into(), "living".into()],
        )
        .unwrap();
        assert_eq!(
            plan,
            GroupPlan::GroupMany {
                target_player: "kitchen".into(),
                player_ids: vec!["kitchen".into(), "living".into()],
            }
        );
        assert_eq!(plan.action(), "group_many");
        assert_eq!(plan.player_id(), "kitchen");

        let grouped = vec![
            player("kitchen", "player", &["living"]),
            player("living", "player", &[]),
            player("den", "player", &[]),
        ];
        let plan = plan_group(
            &grouped,
            &["kitchen".into(), "living".into(), "den".into()],
        )
        .unwrap();
        assert_eq!(
            plan,
            GroupPlan::SetMembers {
                player_id: "kitchen".into(),
                add: vec!["den".into()],
                remove: vec![],
            }
        );
        assert!(plan_group(&grouped, &["kitchen".into(), "living".into()]).is_err());
        assert_eq!(
            plan_group(&players, &["kitchen".into()]),
            Err(GroupPlanError::NeedTwo)
        );
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn choose_playback_prefers_focus_then_playing() {
        let devices = vec![
            candidate("ma:1:kitchen", 4, false, 10),
            candidate("ma:1:living", 9, true, 5),
            candidate("web", 3, true, 99),
            candidate("ma:1:idle", 0, false, 100),
        ];
        let chosen = choose_playback(&devices, Some("ma:1:kitchen")).unwrap();
        assert_eq!(chosen.device_id, "ma:1:kitchen");
        let chosen = choose_playback(&devices, None).unwrap();
        assert_eq!(chosen.device_id, "ma:1:living");
        let paused = vec![
            candidate("ma:1:kitchen", 4, false, 10),
            candidate("ma:1:den", 8, false, 20),
        ];
        assert_eq!(
            choose_playback(&paused, None).unwrap().device_id,
            "ma:1:den"
        );
        assert!(choose_playback(&[], None).is_none());
        assert!(show_ma_now_playing(true, false, &devices, None));
        assert!(!show_ma_now_playing(true, true, &devices, None));
        assert!(!show_ma_now_playing(false, false, &devices, None));
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn transport_stays_when_sonos_reports_idle() {
        assert_eq!(transport_shown("playing", true, false), Some(true));
        assert_eq!(transport_shown("paused", true, false), Some(false));
        assert_eq!(transport_shown("idle", true, false), Some(false));
        assert_eq!(transport_shown("idle", true, true), Some(true));
        assert_eq!(transport_shown("", true, false), Some(false));
        assert_eq!(transport_shown("idle", false, false), None);
        assert_eq!(transport_shown("unknown", false, true), Some(true));
        assert!(!include_in_other_devices("music_assistant"));
        assert!(!include_in_other_devices("Music_Assistant"));
        assert!(include_in_other_devices("Web"));
        assert!(include_in_other_devices("iOS"));
        assert!(include_in_other_devices("Android"));
        assert!(include_in_other_devices("Firewood"));
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn synced_speaker_follows_the_leader_state() {
        let mut member = player("bed", "player", &[]);
        member.state = "idle".into();
        member.synced_to = Some("living".into());
        let mut leader = player("living", "player", &["bed"]);
        leader.state = "playing".into();
        assert_eq!(playback_state_for(&member, &[member.clone(), leader.clone()]), "playing");
        assert_eq!(playback_state_for(&leader, &[member, leader]), "playing");
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn displayed_position_follows_speed_and_stops_when_stale() {
        assert_eq!(displayed_position(10.0, true, 2.0, 100, 105), 20.0);
        assert_eq!(displayed_position(10.0, false, 2.0, 100, 105), 10.0);
        assert_eq!(displayed_position(10.0, true, 1.0, 100, 140), 10.0);
        assert_eq!(displayed_position(10.0, true, 1.0, 0, 140), 10.0);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn players_json_parses_groups() {
        let raw = r#"{
            "enabled": true,
            "connected": true,
            "players": [{
                "player_id": "kitchen",
                "device_id": "ma:1:kitchen",
                "name": "Kitchen",
                "kind": "group",
                "available": true,
                "state": "playing",
                "volume_level": 20,
                "volume_muted": false,
                "group_members": ["living"],
                "synced_to": null,
                "can_group_with": ["den"],
                "queue_id": "kitchen"
            }]
        }"#;
        let parsed: MaPlayersResponse = serde_json::from_str(raw).unwrap();
        assert!(parsed.players[0].is_group());
        assert_eq!(parsed.players[0].group_members, vec!["living".to_string()]);
    }
}
