//! Choose a playable `podcast:alternateEnclosure` (or the RSS enclosure) and verify SRI hashes.
//!
//! Auto-selection follows the namespace: an alternate marked `default="true"` wins when it has a
//! playable `http`/`https` source. Otherwise the RSS enclosure is the default. IPFS and torrent
//! sources stay in the option list so clients can show them, and they are never played or
//! downloaded. HLS and DASH are playable in clients that support them and are not saved as a
//! single file. Music Assistant prefers a progressive file a speaker can fetch.

use base64::engine::general_purpose::{STANDARD, URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256, Sha384, Sha512};

use sqlx::Row;

use crate::database::DatabasePool;
use crate::error::{AppError, AppResult};
use crate::services::podcasting2::AlternateEnclosure;

/// Who is about to play the episode. Music Assistant speakers want a progressive file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Audience {
    Player,
    MusicAssistant,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, utoipa::ToSchema)]
pub struct MediaSource {
    pub uri: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    pub scheme: String,
    pub playable: bool,
    pub downloadable: bool,
    /// `ipfs`, `torrent`, `unsupported_scheme`, or `unsupported_type` when it cannot be played.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unsupported_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, utoipa::ToSchema)]
pub struct MediaIntegrity {
    #[serde(rename = "type")]
    pub kind: String,
    pub value: String,
    /// True when a full-file SRI (sha256/sha384/sha512) check is possible after download.
    pub verifiable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, utoipa::ToSchema)]
pub struct MediaOption {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub mime_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bitrate: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codecs: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rel: Option<String>,
    pub default: bool,
    /// `audio`, `video`, `hls`, `dash`, or `other`.
    pub kind: String,
    pub sources: Vec<MediaSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integrity: Option<MediaIntegrity>,
    pub playable: bool,
    pub downloadable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, utoipa::ToSchema)]
pub struct PlaybackStep {
    pub option_id: String,
    pub source_index: u32,
    pub uri: String,
    pub mime_type: String,
    pub kind: String,
    pub is_video: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, utoipa::ToSchema)]
pub struct DownloadStep {
    pub option_id: String,
    pub source_index: u32,
    pub uri: String,
    pub mime_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integrity: Option<MediaIntegrity>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, utoipa::ToSchema)]
pub struct MediaPlan {
    pub options: Vec<MediaOption>,
    pub selected_id: String,
    /// Playable sources, selected option first. Empty when nothing can be played.
    pub playback: Vec<PlaybackStep>,
    /// Progressive files only. An explicit choice that is HLS/IPFS/torrent yields an empty list.
    pub download: Vec<DownloadStep>,
}

#[derive(Debug, Clone)]
pub struct PrimaryEnclosure {
    pub uri: String,
    pub mime_type: Option<String>,
    pub is_video: bool,
}

pub fn plan_media(
    primary: &PrimaryEnclosure,
    alternates: &[AlternateEnclosure],
    prefer_option_id: Option<&str>,
    audience: Audience,
) -> MediaPlan {
    let mut options = Vec::with_capacity(alternates.len() + 1);
    options.push(primary_option(primary));
    for (i, alt) in alternates.iter().enumerate() {
        options.push(alternate_option(i, alt));
    }
    let explicit = prefer_option_id
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .filter(|id| options.iter().any(|o| o.id == *id));
    let selected_id = select_id(&options, explicit, audience);
    let playback = playback_chain(&options, &selected_id);
    let download = download_chain(&options, &selected_id, explicit.is_some());
    MediaPlan {
        options,
        selected_id,
        playback,
        download,
    }
}

/// Load the RSS enclosure and stored alternate enclosures for an episode the user owns.
pub async fn plan_for_episode(
    db: &DatabasePool,
    episode_id: i32,
    user_id: i32,
    prefer_option_id: Option<&str>,
    audience: Audience,
) -> AppResult<MediaPlan> {
    let (uri, is_video) = load_primary(db, episode_id, user_id).await?;
    let tags = match crate::services::people::store::episode_extras(db, episode_id).await {
        Ok(tags) => tags,
        Err(e) => {
            tracing::warn!("alternate enclosures for episode {episode_id} could not be loaded: {e}");
            Default::default()
        }
    };
    Ok(plan_media(
        &PrimaryEnclosure {
            uri,
            mime_type: None,
            is_video,
        },
        &tags.alternate_enclosures,
        prefer_option_id,
        audience,
    ))
}

async fn load_primary(db: &DatabasePool, episode_id: i32, user_id: i32) -> AppResult<(String, bool)> {
    match db {
        DatabasePool::Postgres(pool) => {
            let row = sqlx::query(
                r#"SELECT e.episodeurl AS episodeurl, COALESCE(e.is_video, FALSE) AS is_video
                   FROM "Episodes" e
                   JOIN "Podcasts" p ON p.feedid = e.feedid
                   WHERE e.episodeid = $1 AND p.userid = $2
                   LIMIT 1"#,
            )
            .bind(episode_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| AppError::not_found("Episode not found"))?;
            let url: String = row.try_get("episodeurl")?;
            let is_video: bool = row.try_get("is_video").unwrap_or(false);
            Ok((url, is_video))
        }
        DatabasePool::MySQL(pool) => {
            let row = sqlx::query(
                "SELECT e.EpisodeURL AS episodeurl, COALESCE(e.IsVideo, 0) AS is_video
                 FROM Episodes e
                 JOIN Podcasts p ON p.FeedID = e.FeedID
                 WHERE e.EpisodeID = ? AND p.UserID = ?
                 LIMIT 1",
            )
            .bind(episode_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| AppError::not_found("Episode not found"))?;
            let url: String = row.try_get("episodeurl")?;
            let is_video = row
                .try_get::<bool, _>("is_video")
                .or_else(|_| row.try_get::<i8, _>("is_video").map(|n| n != 0))
                .unwrap_or(false);
            Ok((url, is_video))
        }
    }
}

fn primary_option(primary: &PrimaryEnclosure) -> MediaOption {
    let mime = primary
        .mime_type
        .clone()
        .filter(|m| !m.trim().is_empty())
        .unwrap_or_else(|| {
            if primary.is_video {
                "video/mp4".to_string()
            } else {
                String::new()
            }
        });
    let source = classify_source(&primary.uri, Some(mime.as_str()));
    let kind = if primary.is_video && source.playable && kind_of(&mime, &primary.uri) == "audio" {
        "video".to_string()
    } else {
        kind_of(&mime, &primary.uri)
    };
    let playable = source.playable && kind != "other";
    let downloadable = source.downloadable && (kind == "audio" || kind == "video");
    MediaOption {
        id: "primary".into(),
        title: None,
        mime_type: if mime.is_empty() {
            match kind.as_str() {
                "video" => "video/mp4".into(),
                "hls" => "application/vnd.apple.mpegurl".into(),
                "dash" => "application/dash+xml".into(),
                _ => "audio/mpeg".into(),
            }
        } else {
            mime
        },
        bitrate: None,
        height: None,
        lang: None,
        codecs: None,
        rel: None,
        default: false,
        kind,
        playable,
        downloadable,
        integrity: None,
        sources: vec![source],
    }
}

fn alternate_option(index: usize, alt: &AlternateEnclosure) -> MediaOption {
    let mime = alt.mime_type.clone();
    let sources: Vec<MediaSource> = if alt.sources.is_empty() {
        Vec::new()
    } else {
        alt.sources
            .iter()
            .map(|s| classify_source(&s.uri, s.content_type.as_deref().or(Some(mime.as_str()))))
            .collect()
    };
    let kind = kind_of(&mime, sources.first().map(|s| s.uri.as_str()).unwrap_or(""));
    let playable = sources.iter().any(|s| s.playable) && kind != "other";
    let downloadable = sources.iter().any(|s| s.downloadable) && (kind == "audio" || kind == "video");
    // A source can be video even when the enclosure mime is audio if content_type says so.
    let kind = if sources.iter().any(|s| s.playable && kind_of(s.content_type.as_deref().unwrap_or(&mime), &s.uri) == "video")
        && kind == "audio"
    {
        "video".to_string()
    } else {
        kind
    };
    let integrity = alt.integrity.as_ref().map(|i| {
        let view = MediaIntegrity {
            kind: i.kind.clone(),
            value: i.value.clone(),
            verifiable: false,
        };
        let verifiable = sri_checker(&view).is_some();
        MediaIntegrity {
            verifiable,
            ..view
        }
    });
    MediaOption {
        id: format!("alt-{index}"),
        title: alt.title.clone().filter(|t| !t.trim().is_empty()),
        mime_type: mime,
        bitrate: alt.bitrate.as_deref().and_then(parse_i64),
        height: alt.height.as_deref().and_then(parse_i64),
        lang: alt.lang.clone().filter(|t| !t.trim().is_empty()),
        codecs: alt.codecs.clone().filter(|t| !t.trim().is_empty()),
        rel: alt.rel.clone().filter(|t| !t.trim().is_empty()),
        default: alt.default == Some(true),
        kind,
        sources,
        integrity,
        playable,
        downloadable,
    }
}

fn classify_source(uri: &str, content_type: Option<&str>) -> MediaSource {
    let uri = uri.trim().to_string();
    let scheme = uri
        .split_once(':')
        .map(|(s, _)| s.to_ascii_lowercase())
        .unwrap_or_default();
    let mime = content_type.unwrap_or("").trim().to_string();
    let kind = kind_of(&mime, &uri);
    let torrent = is_torrent(&scheme, &mime, &uri);
    let ipfs = scheme == "ipfs" || scheme == "ipns";
    let http = scheme == "http" || scheme == "https";
    let (playable, downloadable, reason) = if uri.is_empty() {
        (false, false, Some("unsupported_scheme".into()))
    } else if ipfs {
        (false, false, Some("ipfs".into()))
    } else if torrent {
        (false, false, Some("torrent".into()))
    } else if !http {
        (false, false, Some("unsupported_scheme".into()))
    } else if kind == "other" {
        (false, false, Some("unsupported_type".into()))
    } else if kind == "hls" || kind == "dash" {
        (true, false, None)
    } else {
        (true, true, None)
    };
    MediaSource {
        uri,
        content_type: if mime.is_empty() { None } else { Some(mime) },
        scheme,
        playable,
        downloadable,
        unsupported_reason: reason,
    }
}

fn kind_of(mime: &str, uri: &str) -> String {
    let mime = mime.trim().to_ascii_lowercase();
    let path = uri
        .split(['?', '#'])
        .next()
        .unwrap_or(uri)
        .to_ascii_lowercase();
    if mime.contains("mpegurl")
        || mime.contains("vnd.apple.mpegurl")
        || mime == "audio/x-mpegurl"
        || path.ends_with(".m3u8")
        || path.ends_with(".m3u")
    {
        return "hls".into();
    }
    if mime.contains("dash") || path.ends_with(".mpd") {
        return "dash".into();
    }
    if mime.starts_with("video/") || has_ext(&path, &["mp4", "webm", "mkv", "mov", "m4v"]) {
        return "video".into();
    }
    if mime.starts_with("audio/")
        || has_ext(&path, &["mp3", "m4a", "aac", "ogg", "opus", "flac", "wav", "oga"])
    {
        return "audio".into();
    }
    if (mime.is_empty() || mime == "application/octet-stream") && (path.starts_with("http://") || path.starts_with("https://") || uri.starts_with("http"))
    {
        // RSS enclosures often omit a type. Treat a bare http(s) URL as audio so existing
        // downloads keep working; a video flag on the primary enclosure overrides this.
        return "audio".into();
    }
    "other".into()
}

fn has_ext(path: &str, exts: &[&str]) -> bool {
    exts.iter().any(|ext| path.ends_with(&format!(".{ext}")))
}

fn is_torrent(scheme: &str, mime: &str, uri: &str) -> bool {
    let mime = mime.to_ascii_lowercase();
    let uri = uri.to_ascii_lowercase();
    scheme == "magnet"
        || scheme == "torrent"
        || mime == "application/x-bittorrent"
        || uri.split(['?', '#']).next().unwrap_or(&uri).ends_with(".torrent")
}

fn parse_i64(raw: &str) -> Option<i64> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    trimmed
        .parse::<i64>()
        .ok()
        .or_else(|| trimmed.parse::<f64>().ok().map(|n| n as i64))
}

fn select_id(options: &[MediaOption], explicit: Option<&str>, audience: Audience) -> String {
    if let Some(id) = explicit {
        if let Some(opt) = options.iter().find(|o| o.id == id && acceptable(o, audience)) {
            return opt.id.clone();
        }
        if audience == Audience::Player {
            if let Some(opt) = options.iter().find(|o| o.id == id && o.playable) {
                return opt.id.clone();
            }
        }
    }
    if audience == Audience::MusicAssistant {
        if let Some(opt) = options
            .iter()
            .filter(|o| o.id != "primary" && o.default && o.downloadable)
            .max_by_key(|o| audio_rank(o))
        {
            return opt.id.clone();
        }
        if let Some(opt) = options.iter().find(|o| o.id == "primary" && o.downloadable) {
            return opt.id.clone();
        }
        if let Some(opt) = options
            .iter()
            .filter(|o| o.downloadable)
            .max_by_key(|o| audio_rank(o))
        {
            return opt.id.clone();
        }
    }
    if let Some(opt) = options
        .iter()
        .find(|o| o.id != "primary" && o.default && o.playable)
    {
        return opt.id.clone();
    }
    if let Some(opt) = options.iter().find(|o| o.id == "primary" && o.playable) {
        return opt.id.clone();
    }
    for kind in ["audio", "video", "hls", "dash"] {
        if let Some(opt) = options.iter().find(|o| o.playable && o.kind == kind) {
            return opt.id.clone();
        }
    }
    options
        .first()
        .map(|o| o.id.clone())
        .unwrap_or_else(|| "primary".into())
}

fn acceptable(opt: &MediaOption, audience: Audience) -> bool {
    match audience {
        Audience::Player => opt.playable,
        // An explicit HLS choice is still playable; auto-selection prefers progressive.
        Audience::MusicAssistant => opt.playable,
    }
}

fn audio_rank(opt: &MediaOption) -> i64 {
    let kind = match opt.kind.as_str() {
        "audio" => 2_000_000_000,
        "video" => 1_000_000_000,
        _ => 0,
    };
    kind + opt.bitrate.unwrap_or(0).min(1_000_000_000)
}

fn playback_chain(options: &[MediaOption], selected_id: &str) -> Vec<PlaybackStep> {
    let mut order: Vec<&MediaOption> = Vec::new();
    if let Some(sel) = options.iter().find(|o| o.id == selected_id) {
        order.push(sel);
    }
    let mut rest: Vec<&MediaOption> = options
        .iter()
        .filter(|o| o.id != selected_id && o.playable)
        .collect();
    rest.sort_by_key(|o| fallback_rank(o));
    order.extend(rest);
    let mut seen = std::collections::HashSet::new();
    let mut steps = Vec::new();
    for opt in order {
        for (idx, source) in opt.sources.iter().enumerate() {
            if !source.playable || !seen.insert(source.uri.clone()) {
                continue;
            }
            let mime = source
                .content_type
                .clone()
                .filter(|m| !m.is_empty())
                .unwrap_or_else(|| opt.mime_type.clone());
            let kind = kind_of(&mime, &source.uri);
            let kind = if kind == "audio" && opt.kind == "video" {
                "video".to_string()
            } else if kind == "other" {
                opt.kind.clone()
            } else {
                kind
            };
            steps.push(PlaybackStep {
                option_id: opt.id.clone(),
                source_index: idx as u32,
                uri: source.uri.clone(),
                mime_type: mime,
                kind: kind.clone(),
                is_video: kind == "video" || (opt.height.unwrap_or(0) > 0 && kind != "audio"),
            });
        }
    }
    steps
}

fn fallback_rank(opt: &MediaOption) -> u8 {
    match opt.kind.as_str() {
        "audio" if opt.downloadable => 0,
        "video" if opt.downloadable => 1,
        "audio" => 2,
        "video" => 3,
        "hls" => 4,
        "dash" => 5,
        _ => 6,
    }
}

fn download_chain(options: &[MediaOption], selected_id: &str, explicit: bool) -> Vec<DownloadStep> {
    let mut order: Vec<&MediaOption> = Vec::new();
    if let Some(sel) = options.iter().find(|o| o.id == selected_id) {
        order.push(sel);
    }
    if !explicit {
        let mut rest: Vec<&MediaOption> = options
            .iter()
            .filter(|o| o.id != selected_id && o.downloadable)
            .collect();
        rest.sort_by_key(|o| fallback_rank(o));
        order.extend(rest);
    }
    let mut seen = std::collections::HashSet::new();
    let mut steps = Vec::new();
    for opt in order {
        for (idx, source) in opt.sources.iter().enumerate() {
            if !source.downloadable || !seen.insert(source.uri.clone()) {
                continue;
            }
            steps.push(DownloadStep {
                option_id: opt.id.clone(),
                source_index: idx as u32,
                uri: source.uri.clone(),
                mime_type: source
                    .content_type
                    .clone()
                    .filter(|m| !m.is_empty())
                    .unwrap_or_else(|| opt.mime_type.clone()),
                integrity: opt.integrity.clone(),
            });
        }
    }
    steps
}

pub fn file_extension(mime: &str, uri: &str) -> &'static str {
    let kind = kind_of(mime, uri);
    let path = uri.split(['?', '#']).next().unwrap_or(uri).to_ascii_lowercase();
    let mime = mime.to_ascii_lowercase();
    if path.ends_with(".m4a") || mime.contains("mp4") && kind == "audio" || mime.contains("aac") {
        return "m4a";
    }
    if path.ends_with(".opus") || mime.contains("opus") {
        return "opus";
    }
    if path.ends_with(".ogg") || path.ends_with(".oga") || mime.contains("ogg") {
        return "ogg";
    }
    if path.ends_with(".flac") || mime.contains("flac") {
        return "flac";
    }
    if path.ends_with(".wav") || mime.contains("wav") {
        return "wav";
    }
    if path.ends_with(".webm") || mime.contains("webm") {
        return "webm";
    }
    if path.ends_with(".mkv") {
        return "mkv";
    }
    if kind == "video" || path.ends_with(".mp4") || path.ends_with(".m4v") {
        return "mp4";
    }
    "mp3"
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Algo {
    Sha256,
    Sha384,
    Sha512,
}

impl Algo {
    fn len(self) -> usize {
        match self {
            Algo::Sha256 => 32,
            Algo::Sha384 => 48,
            Algo::Sha512 => 64,
        }
    }

    fn rank(self) -> u8 {
        match self {
            Algo::Sha256 => 1,
            Algo::Sha384 => 2,
            Algo::Sha512 => 3,
        }
    }
}

struct ExpectedHash {
    algo: Algo,
    digests: Vec<Vec<u8>>,
}

pub struct RunningIntegrity {
    expected: Option<ExpectedHash>,
    sha256: Sha256,
    sha384: Sha384,
    sha512: Sha512,
}

impl RunningIntegrity {
    pub fn from_integrity(integrity: Option<&MediaIntegrity>) -> Self {
        Self {
            expected: integrity.and_then(sri_checker),
            sha256: Sha256::new(),
            sha384: Sha384::new(),
            sha512: Sha512::new(),
        }
    }

    pub fn verifiable(&self) -> bool {
        self.expected.is_some()
    }

    pub fn update(&mut self, chunk: &[u8]) {
        if self.expected.is_none() {
            return;
        }
        self.sha256.update(chunk);
        self.sha384.update(chunk);
        self.sha512.update(chunk);
    }

    /// `None` when there is nothing to check. `Some(false)` is a mismatch.
    pub fn matches(&self) -> Option<bool> {
        let expected = self.expected.as_ref()?;
        let digest = match expected.algo {
            Algo::Sha256 => self.sha256.clone().finalize().to_vec(),
            Algo::Sha384 => self.sha384.clone().finalize().to_vec(),
            Algo::Sha512 => self.sha512.clone().finalize().to_vec(),
        };
        Some(expected.digests.iter().any(|d| d == &digest))
    }
}

fn sri_checker(integrity: &MediaIntegrity) -> Option<ExpectedHash> {
    let kind = integrity.kind.trim().to_ascii_lowercase();
    let tokens = if kind == "sri" || kind == "subresourceintegrity" || kind.is_empty() {
        parse_sri_tokens(&integrity.value)
    } else if matches!(kind.as_str(), "sha256" | "sha384" | "sha512") {
        parse_bare_digest(&kind, &integrity.value)
    } else {
        return None;
    };
    if tokens.is_empty() {
        return None;
    }
    let strongest = tokens.iter().map(|t| t.0.rank()).max()?;
    let algo = tokens.iter().find(|t| t.0.rank() == strongest)?.0;
    let digests = tokens
        .into_iter()
        .filter(|(a, _)| *a == algo)
        .map(|(_, d)| d)
        .collect();
    Some(ExpectedHash { algo, digests })
}

fn parse_sri_tokens(value: &str) -> Vec<(Algo, Vec<u8>)> {
    value
        .split_whitespace()
        .filter_map(|token| {
            let (name, payload) = token.split_once('-')?;
            let algo = algo_from_name(name)?;
            let bytes = decode_digest(payload)?;
            if bytes.len() != algo.len() {
                return None;
            }
            Some((algo, bytes))
        })
        .collect()
}

fn parse_bare_digest(kind: &str, value: &str) -> Vec<(Algo, Vec<u8>)> {
    let algo = match algo_from_name(kind) {
        Some(a) => a,
        None => return Vec::new(),
    };
    let value = value.trim();
    if let Some((name, payload)) = value.split_once('-') {
        if algo_from_name(name) == Some(algo) {
            if let Some(bytes) = decode_digest(payload) {
                if bytes.len() == algo.len() {
                    return vec![(algo, bytes)];
                }
            }
        }
    }
    if let Some(bytes) = decode_digest(value) {
        if bytes.len() == algo.len() {
            return vec![(algo, bytes)];
        }
    }
    if let Some(bytes) = hex_decode(value) {
        if bytes.len() == algo.len() {
            return vec![(algo, bytes)];
        }
    }
    Vec::new()
}

fn algo_from_name(name: &str) -> Option<Algo> {
    match name.trim().to_ascii_lowercase().as_str() {
        "sha256" => Some(Algo::Sha256),
        "sha384" => Some(Algo::Sha384),
        "sha512" => Some(Algo::Sha512),
        _ => None,
    }
}

fn decode_digest(payload: &str) -> Option<Vec<u8>> {
    STANDARD
        .decode(payload)
        .ok()
        .or_else(|| URL_SAFE.decode(payload).ok())
        .or_else(|| URL_SAFE_NO_PAD.decode(payload).ok())
}

fn hex_decode(value: &str) -> Option<Vec<u8>> {
    let value = value.trim();
    if value.len() % 2 != 0 || !value.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    (0..value.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&value[i..i + 2], 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::podcasting2::{Integrity, Source};

    fn alt(
        mime: &str,
        default: bool,
        uris: &[&str],
        integrity: Option<(&str, &str)>,
    ) -> AlternateEnclosure {
        AlternateEnclosure {
            mime_type: mime.into(),
            default: Some(default),
            sources: uris
                .iter()
                .map(|u| Source {
                    uri: (*u).into(),
                    content_type: None,
                })
                .collect(),
            integrity: integrity.map(|(k, v)| Integrity {
                kind: k.into(),
                value: v.into(),
            }),
            title: Some("Low".into()),
            bitrate: Some("64000".into()),
            ..AlternateEnclosure::default()
        }
    }

    fn primary(uri: &str) -> PrimaryEnclosure {
        PrimaryEnclosure {
            uri: uri.into(),
            mime_type: Some("audio/mpeg".into()),
            is_video: false,
        }
    }

    #[test]
    fn default_alternate_beats_the_rss_enclosure() {
        let plan = plan_media(
            &primary("https://cdn.example/ep.mp3"),
            &[alt(
                "audio/mpeg",
                true,
                &["https://mirror.example/ep.mp3", "ipfs://bafy"],
                None,
            )],
            None,
            Audience::Player,
        );
        assert_eq!(plan.selected_id, "alt-0");
        assert_eq!(plan.playback[0].uri, "https://mirror.example/ep.mp3");
        assert!(plan.playback.iter().all(|s| s.uri.starts_with("https://")));
        let ipfs = &plan.options[1].sources[1];
        assert!(!ipfs.playable);
        assert_eq!(ipfs.unsupported_reason.as_deref(), Some("ipfs"));
    }

    #[test]
    fn rss_enclosure_wins_when_nothing_is_marked_default() {
        let plan = plan_media(
            &primary("https://cdn.example/ep.mp3"),
            &[alt("audio/mpeg", false, &["https://mirror.example/low.mp3"], None)],
            None,
            Audience::Player,
        );
        assert_eq!(plan.selected_id, "primary");
        assert_eq!(plan.playback[0].uri, "https://cdn.example/ep.mp3");
        assert_eq!(plan.playback[1].uri, "https://mirror.example/low.mp3");
    }

    #[test]
    fn torrent_is_shown_and_not_played_and_fallback_skips_it() {
        let plan = plan_media(
            &primary("magnet:?xt=urn:btih:abc"),
            &[alt(
                "audio/mpeg",
                false,
                &[
                    "https://cdn.example/a.mp3",
                    "https://cdn.example/a.torrent",
                ],
                None,
            )],
            None,
            Audience::Player,
        );
        assert!(!plan.options[0].playable);
        assert_eq!(
            plan.options[0].sources[0].unsupported_reason.as_deref(),
            Some("torrent")
        );
        assert_eq!(plan.selected_id, "alt-0");
        assert_eq!(plan.playback.len(), 1);
        assert_eq!(plan.playback[0].uri, "https://cdn.example/a.mp3");
        assert_eq!(
            plan.options[1].sources[1].unsupported_reason.as_deref(),
            Some("torrent")
        );
    }

    #[test]
    fn explicit_choice_is_honored_and_limits_the_download() {
        let plan = plan_media(
            &primary("https://cdn.example/ep.mp3"),
            &[
                alt("audio/mpeg", true, &["https://mirror.example/def.mp3"], None),
                alt(
                    "audio/mp4",
                    false,
                    &["https://cdn.example/hq.m4a", "https://backup.example/hq.m4a"],
                    None,
                ),
            ],
            Some("alt-1"),
            Audience::Player,
        );
        assert_eq!(plan.selected_id, "alt-1");
        assert_eq!(plan.download.len(), 2);
        assert!(plan.download.iter().all(|d| d.option_id == "alt-1"));
        assert_eq!(plan.playback[0].option_id, "alt-1");
        assert!(plan.playback.iter().any(|s| s.option_id == "primary"));
    }

    #[test]
    fn hls_choice_is_not_a_file_download() {
        let plan = plan_media(
            &primary("https://cdn.example/ep.mp3"),
            &[alt(
                "application/vnd.apple.mpegurl",
                true,
                &["https://cdn.example/live.m3u8"],
                None,
            )],
            Some("alt-0"),
            Audience::Player,
        );
        assert!(plan.options[1].playable);
        assert!(!plan.options[1].downloadable);
        assert!(plan.download.is_empty());
        assert_eq!(plan.playback[0].kind, "hls");
        assert!(!plan.playback[0].is_video);
    }

    #[test]
    fn music_assistant_prefers_a_progressive_file_over_default_hls() {
        let plan = plan_media(
            &primary("https://cdn.example/ep.mp3"),
            &[alt(
                "application/vnd.apple.mpegurl",
                true,
                &["https://cdn.example/live.m3u8"],
                None,
            )],
            None,
            Audience::MusicAssistant,
        );
        assert_eq!(plan.selected_id, "primary");
        assert_eq!(plan.playback[0].uri, "https://cdn.example/ep.mp3");
    }

    #[test]
    fn music_assistant_uses_hls_when_nothing_else_plays() {
        let plan = plan_media(
            &PrimaryEnclosure {
                uri: "ipfs://bafy".into(),
                mime_type: None,
                is_video: false,
            },
            &[alt(
                "application/vnd.apple.mpegurl",
                false,
                &["https://cdn.example/a.m3u8"],
                None,
            )],
            None,
            Audience::MusicAssistant,
        );
        assert_eq!(plan.selected_id, "alt-0");
        assert_eq!(plan.playback[0].kind, "hls");
    }

    #[test]
    fn auto_download_falls_back_to_another_progressive_source() {
        let plan = plan_media(
            &primary("https://cdn.example/ep.mp3"),
            &[alt(
                "audio/mpeg",
                true,
                &["https://mirror.example/ep.mp3"],
                None,
            )],
            None,
            Audience::Player,
        );
        assert_eq!(plan.download[0].uri, "https://mirror.example/ep.mp3");
        assert_eq!(plan.download[1].uri, "https://cdn.example/ep.mp3");
    }

    #[test]
    fn video_height_marks_hls_as_video() {
        let mut enclosure = alt(
            "application/vnd.apple.mpegurl",
            true,
            &["https://cdn.example/v.m3u8"],
            None,
        );
        enclosure.height = Some("720".into());
        let plan = plan_media(&primary("https://cdn.example/ep.mp3"), &[enclosure], None, Audience::Player);
        assert!(plan.playback[0].is_video);
    }

    #[test]
    fn sri_matches_sha384_and_rejects_a_different_file() {
        let bytes = b"pinepods-enclosure";
        let digest = Sha384::digest(bytes);
        let token = format!("sha384-{}", STANDARD.encode(digest));
        let integrity = MediaIntegrity {
            kind: "sri".into(),
            value: token,
            verifiable: true,
        };
        let mut ok = RunningIntegrity::from_integrity(Some(&integrity));
        ok.update(bytes);
        assert_eq!(ok.matches(), Some(true));
        let mut bad = RunningIntegrity::from_integrity(Some(&integrity));
        bad.update(b"other-bytes");
        assert_eq!(bad.matches(), Some(false));
    }

    #[test]
    fn sri_uses_the_strongest_algorithm_present() {
        let bytes = b"abc";
        let sha256 = format!("sha256-{}", STANDARD.encode(Sha256::digest(bytes)));
        let sha512 = format!("sha512-{}", STANDARD.encode(Sha512::digest(b"not-the-file")));
        // Strongest listed algorithm is sha512, and it does not match `bytes`.
        let integrity = MediaIntegrity {
            kind: "SRI".into(),
            value: format!("{sha256} {sha512}"),
            verifiable: true,
        };
        let mut check = RunningIntegrity::from_integrity(Some(&integrity));
        check.update(bytes);
        assert_eq!(check.matches(), Some(false));

        let sha512_ok = format!("sha512-{}", STANDARD.encode(Sha512::digest(bytes)));
        let integrity = MediaIntegrity {
            kind: "sri".into(),
            value: format!("{sha256} {sha512_ok}"),
            verifiable: true,
        };
        let mut check = RunningIntegrity::from_integrity(Some(&integrity));
        check.update(bytes);
        assert_eq!(check.matches(), Some(true));
    }

    #[test]
    fn hex_sha256_integrity_type_is_checked() {
        let bytes = b"abc";
        let hex = format!("{:x}", Sha256::digest(bytes));
        let integrity = MediaIntegrity {
            kind: "sha256".into(),
            value: hex,
            verifiable: true,
        };
        let mut check = RunningIntegrity::from_integrity(Some(&integrity));
        check.update(bytes);
        assert_eq!(check.matches(), Some(true));
    }

    #[test]
    fn unknown_integrity_type_is_not_verifiable() {
        let integrity = MediaIntegrity {
            kind: "pgp".into(),
            value: "deadbeef".into(),
            verifiable: false,
        };
        let check = RunningIntegrity::from_integrity(Some(&integrity));
        assert!(!check.verifiable());
        assert_eq!(check.matches(), None);
    }

    #[test]
    fn file_extension_follows_mime() {
        assert_eq!(file_extension("audio/mp4", "https://cdn/a"), "m4a");
        assert_eq!(file_extension("video/mp4", "https://cdn/a"), "mp4");
        assert_eq!(file_extension("audio/mpeg", "https://cdn/a.mp3"), "mp3");
    }
}
