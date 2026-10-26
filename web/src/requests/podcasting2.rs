//! Podcasting 2.0 tag documents from `tags` on the episode and podcast endpoints.
//!
//! Field names match `docs/podcasting2-api.md`. Empty tags deserialize as defaults so a
//! response that only has the older top-level keys still loads.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ItemTags {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enclosure_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chapters_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chapters_type: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub soundbites: Vec<Soundbite>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub locations: Vec<Location>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub season: Option<Season>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub episode: Option<EpisodeNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<License>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub image: Vec<PodcastImage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub images: Option<ImageSet>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content_links: Vec<ContentLink>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub social_interacts: Vec<SocialInteract>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub funding: Vec<FundingLink>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat: Option<Chat>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub value: Vec<ValueBlock>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remote_items: Vec<RemoteItem>,
    /// Kept so a feed that sends them still deserializes. Not shown in the UI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub medium: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<IgnoredTag>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub txts: Vec<IgnoredTag>,
}

/// One `podcast:liveItem` from the show payload. The enclosure URL lives on `tags`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LiveItem {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub start: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enclosure_type: Option<String>,
    /// `podcast:liveValue`. Stored for display. Nothing here sends a payment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub live_value: Option<LiveValue>,
    #[serde(default)]
    pub tags: LiveItemTags,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LiveItemTags {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enclosure_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LivePlayback {
    pub url: String,
    pub hls: bool,
    pub is_video: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ChannelTags {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub medium: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locked: Option<Locked>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub locations: Vec<Location>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trailers: Vec<Trailer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<License>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub image: Vec<PodcastImage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub images: Option<ImageSet>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub social_interacts: Vec<SocialInteract>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_frequency: Option<UpdateFrequency>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher: Option<RemoteItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat: Option<Chat>,
    /// Channel `podcast:complete`. Presence means the show will not add episodes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub complete: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remote_items: Vec<RemoteItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub funding: Vec<FundingLink>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub value: Vec<ValueBlock>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<IgnoredTag>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub txts: Vec<IgnoredTag>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub live_items: Vec<LiveItem>,
}

/// A tag we accept so parsing doesn't fail, and do not render.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IgnoredTag {
    #[serde(default)]
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Locked {
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Soundbite {
    pub start_time: String,
    pub duration: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Location {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rel: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub osm: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Season {
    pub number: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EpisodeNumber {
    pub number: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct License {
    #[serde(default)]
    pub identifier: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PodcastImage {
    pub href: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aspect_ratio: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageSource {
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub descriptor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageSet {
    #[serde(default)]
    pub srcset: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<ImageSource>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContentLink {
    pub href: String,
    #[serde(default)]
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SocialInteract {
    #[serde(default)]
    pub protocol: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FundingLink {
    pub url: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Chat {
    #[serde(default)]
    pub server: String,
    #[serde(default)]
    pub protocol: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub space: Option<String>,
    /// Web client for the room, for example a KiwiIRC link next to an `irc://` server.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embed_url: Option<String>,
}

/// `podcast:liveValue` on a live item. Stored only. Nothing here sends a payment.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LiveValue {
    #[serde(default)]
    pub uri: String,
    #[serde(default)]
    pub protocol: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub value: Vec<ValueBlock>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RemoteItem {
    #[serde(default)]
    pub feed_guid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feed_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_guid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub medium: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValueRecipient {
    #[serde(default)]
    pub name: String,
    #[serde(default, rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub split: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValueTimeSplit {
    #[serde(default)]
    pub start_time: String,
    #[serde(default)]
    pub duration: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_start_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_percentage: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recipients: Vec<ValueRecipient>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_item: Option<RemoteItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValueBlock {
    #[serde(default, rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub suggested: Option<String>,
    #[serde(default)]
    pub recipients: Vec<ValueRecipient>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub time_splits: Vec<ValueTimeSplit>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trailer {
    #[serde(default)]
    pub title: String,
    pub url: String,
    #[serde(default)]
    pub pubdate: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub season: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateFrequency {
    #[serde(default)]
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub complete: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dtstart: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rrule: Option<String>,
}

/// Season and episode numbers after Podcasting 2.0 values win and iTunes fills the gaps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedNumbers {
    pub season_number: Option<i64>,
    pub season_name: Option<String>,
    pub episode_number: Option<i64>,
    /// Podcasting 2.0 `display`, or the numeric episode text when display is absent.
    pub episode_text: Option<String>,
    pub from_podcasting2: bool,
}

impl ResolvedNumbers {
    pub fn is_empty(&self) -> bool {
        self.season_number.is_none() && self.season_name.is_none() && self.episode_text.is_none()
    }
}

/// How often a show says it publishes, parsed from `podcast:updateFrequency`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cadence {
    Hourly(u32),
    Daily(u32),
    Weekly(u32),
    Monthly(u32),
    Yearly(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtworkChoice {
    pub src: String,
    pub srcset: Option<String>,
    pub alt: Option<String>,
}

/// A soundbite window in seconds. `end` is start + duration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClipWindow {
    pub start: f64,
    pub end: f64,
}

pub fn resolve_numbers(
    season: Option<&Season>,
    episode: Option<&EpisodeNumber>,
    itunes_season: Option<i64>,
    itunes_episode: Option<i64>,
) -> ResolvedNumbers {
    let season_number = season
        .and_then(|s| parse_whole(&s.number))
        .or(itunes_season.filter(|n| *n > 0));
    let season_name = season.and_then(|s| {
        s.name
            .as_ref()
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty())
    });
    let (episode_number, episode_text, episode_from_p2) = if let Some(ep) = episode {
        let number = parse_whole(&ep.number).or(itunes_episode.filter(|n| *n > 0));
        let display = ep
            .display
            .as_ref()
            .map(|d| d.trim().to_string())
            .filter(|d| !d.is_empty());
        let text = display.or_else(|| {
            number
                .map(|n| n.to_string())
                .or_else(|| non_empty(&ep.number))
        });
        (number, text, true)
    } else if let Some(n) = itunes_episode.filter(|n| *n > 0) {
        (Some(n), Some(n.to_string()), false)
    } else {
        (None, None, false)
    };
    ResolvedNumbers {
        season_number,
        season_name,
        episode_number,
        episode_text,
        from_podcasting2: season.is_some() || episode_from_p2,
    }
}

/// Sort key: numbered episodes first, then season, then episode. Unnumbered sort last.
pub fn season_sort_key(numbers: &ResolvedNumbers) -> (u8, i64, i64) {
    let unnumbered = if numbers.season_number.is_none() && numbers.episode_number.is_none() {
        1
    } else {
        0
    };
    (
        unnumbered,
        numbers.season_number.unwrap_or(i64::MAX),
        numbers.episode_number.unwrap_or(i64::MAX),
    )
}

/// Group when at least two episodes carry a season number and they are not all the same
/// single bucket of one. A single season still groups so the header can show its name.
pub fn should_group_by_season(numbers: &[ResolvedNumbers]) -> bool {
    numbers.iter().any(|n| n.season_number.is_some() || n.season_name.is_some())
}

pub fn season_group_key(numbers: &ResolvedNumbers) -> Option<i64> {
    numbers.season_number
}

pub fn soundbite_window(start_time: &str, duration: &str) -> Option<ClipWindow> {
    let start = parse_seconds(start_time)?;
    let duration = parse_seconds(duration).unwrap_or(0.0);
    if start < 0.0 || duration < 0.0 {
        return None;
    }
    Some(ClipWindow {
        start,
        end: start + duration,
    })
}

pub fn format_clock(seconds: f64) -> String {
    if !seconds.is_finite() || seconds < 0.0 {
        return "0:00".to_string();
    }
    let total = seconds.round() as i64;
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let secs = total % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{secs:02}")
    } else {
        format!("{minutes}:{secs:02}")
    }
}

/// OpenStreetMap link for a location. Geo coordinates win; an OSM id is the fallback.
pub fn location_map_url(location: &Location) -> Option<String> {
    if let Some(geo) = location.geo.as_deref() {
        if let Some((lat, lon)) = parse_geo(geo) {
            return Some(format!(
                "https://www.openstreetmap.org/?mlat={lat}&mlon={lon}#map=14/{lat}/{lon}"
            ));
        }
    }
    location.osm.as_deref().and_then(osm_url)
}

pub fn chat_url(chat: &Chat) -> Option<String> {
    let server = chat.server.trim();
    let protocol = chat.protocol.trim().to_ascii_lowercase();
    let space = chat.space.as_deref().unwrap_or("").trim();
    if let Some(embed) = chat.embed_url.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        if let Some(url) = http_link(embed) {
            if !is_dangerous(url) {
                return Some(url.to_string());
            }
        }
    }
    if is_dangerous(server) {
        return None;
    }
    if server.starts_with("http://") || server.starts_with("https://") {
        return Some(server.to_string());
    }
    match protocol.as_str() {
        "irc" if !server.is_empty() => {
            let room = space.trim_start_matches('#');
            if room.is_empty() {
                Some(format!("irc://{server}"))
            } else {
                Some(format!("irc://{server}/{room}"))
            }
        }
        "matrix" => {
            let room = if space.starts_with('#') || space.starts_with('!') {
                if space.contains(':') || server.is_empty() {
                    space.to_string()
                } else {
                    format!("{space}:{server}")
                }
            } else if !space.is_empty() && !server.is_empty() {
                format!("#{space}:{server}")
            } else if let Some(id) = chat.account_id.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                id.to_string()
            } else if !server.is_empty() {
                return Some(format!("https://{server}"));
            } else {
                return None;
            };
            Some(format!("https://matrix.to/#/{}", urlencoding::encode(&room)))
        }
        "xmpp" => {
            let room = if !space.is_empty() { space } else { chat.account_id.as_deref().unwrap_or("") };
            if room.is_empty() {
                None
            } else if space.is_empty() {
                Some(format!("xmpp:{room}"))
            } else {
                Some(format!("xmpp:{room}?join"))
            }
        }
        "nostr" => chat.account_id.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(|id| {
            if id.starts_with("npub") {
                format!("https://njump.me/{id}")
            } else {
                format!("nostr:{id}")
            }
        }),
        _ if !server.is_empty() => Some(format!("https://{server}")),
        _ => None,
    }
}

pub fn social_url(social: &SocialInteract) -> Option<String> {
    if social.protocol.eq_ignore_ascii_case("disabled") {
        return None;
    }
    for candidate in [social.uri.as_deref(), social.account_url.as_deref()] {
        if let Some(url) = candidate.map(str::trim).filter(|s| !s.is_empty()) {
            if !is_dangerous(url) && url.contains(':') {
                return Some(url.to_string());
            }
        }
    }
    None
}

pub fn safe_link(url: &str) -> Option<&str> {
    let url = url.trim();
    if url.is_empty() || is_dangerous(url) || !url.contains(':') {
        None
    } else {
        Some(url)
    }
}

pub fn http_link(url: &str) -> Option<&str> {
    let url = url.trim();
    if url.starts_with("https://") || url.starts_with("http://") {
        Some(url)
    } else {
        None
    }
}

/// Start and end of each chapter in seconds. A chapter ends where the next
/// one starts, and the last one at `duration`.
pub fn chapter_bounds(chapters: &[crate::requests::pod_req::Chapter], duration: f64) -> Vec<(f64, f64)> {
    chapters
        .iter()
        .enumerate()
        .map(|(index, chapter)| {
            let start = chapter.startTime.unwrap_or_default() as f64;
            let end = chapters
                .get(index + 1)
                .map(|next| next.startTime.unwrap_or_default() as f64)
                .unwrap_or(duration);
            (start, end)
        })
        .collect()
}

/// How far `position` is through a chapter, in percent: 100 once it is past,
/// 0 before it starts.
pub fn chapter_progress(position: f64, start: f64, end: f64) -> f64 {
    if position >= end {
        100.0
    } else if position >= start && end > start {
        ((position - start) / (end - start) * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    }
}

/// Image of the chapter playing at `position`, when that chapter has one.
/// The cover falls back to the episode artwork otherwise.
pub fn chapter_image_at(chapters: &[crate::requests::pod_req::Chapter], position: f64) -> Option<&str> {
    chapters
        .iter()
        .rev()
        .find(|c| c.startTime.is_some_and(|start| start as f64 <= position))
        .and_then(|c| c.img.as_deref())
}

/// One timed transcript cue parsed from SRT/VTT.
#[derive(Debug, Clone, PartialEq)]
pub struct TranscriptCue {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

/// Parse an SRT/VTT timestamp (`HH:MM:SS,mmm`, `MM:SS.mmm`, …) into seconds.
fn parse_ts(s: &str) -> Option<f64> {
    let s = s.trim().replace(',', ".");
    let parts: Vec<&str> = s.split(':').collect();
    let (h, m, sec) = match parts.as_slice() {
        [h, m, s] => (h.trim().parse::<f64>().ok()?, m.parse::<f64>().ok()?, s.parse::<f64>().ok()?),
        [m, s] => (0.0, m.trim().parse::<f64>().ok()?, s.parse::<f64>().ok()?),
        _ => return None,
    };
    Some(h * 3600.0 + m * 60.0 + sec)
}

/// Parse SRT/VTT content into timed cues, so we can render the transcript as timestamped,
/// clickable segments instead of an undifferentiated text blob (#790). Returns empty for content
/// that has no cue timings (e.g. plain text / HTML), so callers can fall back to paragraphs.
pub fn parse_transcript_cues(content: &str) -> Vec<TranscriptCue> {
    let speaker_re = regex::Regex::new(r"<v\s+[^>]*>").unwrap();
    let mut cues = Vec::new();
    let mut lines = content.lines().peekable();
    while let Some(line) = lines.next() {
        let Some(idx) = line.find("-->") else { continue };
        let start = parse_ts(&line[..idx]);
        let end = parse_ts(line[idx + 3..].split_whitespace().next().unwrap_or(""));
        let (Some(start), Some(end)) = (start, end) else { continue };
        let mut text_parts: Vec<String> = Vec::new();
        while let Some(peek) = lines.peek() {
            if peek.trim().is_empty() || peek.contains("-->") {
                break;
            }
            let l = lines.next().unwrap();
            let l = speaker_re.replace_all(l, "");
            let l = l.replace("</v>", "");
            let t = l.trim();
            if !t.is_empty() {
                text_parts.push(t.to_string());
            }
        }
        let text = text_parts.join(" ");
        if !text.is_empty() {
            cues.push(TranscriptCue { start, end, text });
        }
    }
    cues
}

/// Cue to highlight while playback is at `position`: the last one that has
/// started, so the highlight holds through the short gaps between cues.
pub fn current_cue(cues: &[TranscriptCue], position: f64) -> Option<usize> {
    if !position.is_finite() {
        return None;
    }
    cues.iter().rposition(|cue| cue.start <= position)
}

/// Something that changes whether a following transcript keeps the current
/// cue in view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptFollow {
    /// The listener scrolled the transcript themselves.
    UserScrolled,
    /// The listener tapped a cue's timestamp.
    Seeked,
    /// The listener pressed Follow playback.
    Resumed,
}

/// Whether the transcript follows playback after `event`. Scrolling to read
/// stops it; jumping to a cue or pressing Follow starts it again.
pub fn follows_after(event: TranscriptFollow) -> bool {
    !matches!(event, TranscriptFollow::UserScrolled)
}

pub fn artwork_choice(images: &[PodcastImage], set: Option<&ImageSet>, fallback: &str) -> ArtworkChoice {
    let preferred = images.iter().find(|img| {
        img.purpose
            .as_deref()
            .map(|p| p.eq_ignore_ascii_case("artwork") || p.eq_ignore_ascii_case("cover"))
            .unwrap_or(false)
            && http_link(&img.href).is_some()
    });
    let first = images.iter().find(|img| http_link(&img.href).is_some());
    let chosen = preferred.or(first);
    let srcset = set.and_then(srcset_of);
    let src = chosen
        .map(|img| img.href.trim().to_string())
        .or_else(|| srcset.as_deref().and_then(largest_srcset_url))
        .unwrap_or_else(|| fallback.to_string());
    ArtworkChoice {
        alt: chosen.and_then(|img| img.alt.clone()),
        srcset,
        src,
    }
}

pub fn cadence_of(rrule: &str) -> Option<Cadence> {
    let mut freq: Option<String> = None;
    let mut interval = 1u32;
    for part in rrule.split(';') {
        let mut kv = part.splitn(2, '=');
        let Some(key) = kv.next() else { continue };
        let Some(value) = kv.next() else { continue };
        match key.trim().to_ascii_uppercase().as_str() {
            "FREQ" => freq = Some(value.trim().to_ascii_uppercase()),
            "INTERVAL" => {
                if let Ok(n) = value.trim().parse::<u32>() {
                    if n > 0 {
                        interval = n;
                    }
                }
            }
            _ => {}
        }
    }
    match freq.as_deref() {
        Some("HOURLY") => Some(Cadence::Hourly(interval)),
        Some("DAILY") => Some(Cadence::Daily(interval)),
        Some("WEEKLY") => Some(Cadence::Weekly(interval)),
        Some("MONTHLY") => Some(Cadence::Monthly(interval)),
        Some("YEARLY") => Some(Cadence::Yearly(interval)),
        _ => None,
    }
}

pub fn trailer_is_video(mime: Option<&str>) -> bool {
    mime.unwrap_or("")
        .trim()
        .to_ascii_lowercase()
        .starts_with("video/")
}

/// `pending`, `live`, or `ended`. Any other status is treated as upcoming.
pub fn normalize_live_status(status: &str) -> &'static str {
    match status.trim().to_ascii_lowercase().as_str() {
        "live" => "live",
        "ended" => "ended",
        _ => "pending",
    }
}

pub fn live_stream_is_hls(mime: Option<&str>, url: &str) -> bool {
    let mime = mime.unwrap_or("").trim().to_ascii_lowercase();
    if mime.contains("mpegurl") || mime.contains("m3u8") {
        return true;
    }
    let path = url.split(['?', '#']).next().unwrap_or(url).to_ascii_lowercase();
    path.ends_with(".m3u8") || path.ends_with(".m3u")
}

pub fn live_stream_is_video(mime: Option<&str>) -> bool {
    let mime = mime.unwrap_or("").trim().to_ascii_lowercase();
    mime.starts_with("video/") || mime.contains("dash")
}

/// Playable http(s) enclosure for a live item. Other schemes are left unplayed.
pub fn live_playback(mime: Option<&str>, url: &str) -> Option<LivePlayback> {
    let url = http_link(url)?.to_string();
    Some(LivePlayback {
        hls: live_stream_is_hls(mime, &url),
        is_video: live_stream_is_video(mime),
        url,
    })
}

/// Same visibility rule as the API: hide a live item once it is over.
///
/// `status="ended"` is enough. An end time in the past hides the item too, even
/// when the status is still `live` or `pending` and even when no episode guid
/// matches. `episode_guids` is accepted so existing callers keep compiling.
pub fn visible_live_items(items: &[LiveItem], episode_guids: &[String]) -> Vec<LiveItem> {
    let _ = episode_guids;
    let mut out: Vec<LiveItem> = items
        .iter()
        .filter(|item| !live_item_is_over(item))
        .cloned()
        .collect();
    out.sort_by(|a, b| {
        fn rank(status: &str) -> u8 {
            match normalize_live_status(status) {
                "live" => 0,
                "pending" => 1,
                _ => 2,
            }
        }
        rank(&a.status)
            .cmp(&rank(&b.status))
            .then_with(|| a.start.cmp(&b.start))
    });
    out
}

fn live_item_is_over(item: &LiveItem) -> bool {
    if normalize_live_status(&item.status) == "ended" {
        return true;
    }
    item.end
        .as_deref()
        .and_then(parse_live_end)
        .is_some_and(|end| end < chrono::Utc::now())
}

/// RFC3339, a date, or an offset written without a colon (`+0000`).
fn parse_live_end(raw: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    use chrono::{DateTime, NaiveDate, TimeZone, Utc};
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(raw) {
        return Some(dt.with_timezone(&Utc));
    }
    if raw.len() == 10 {
        if let Ok(d) = NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
            return Some(Utc.from_utc_datetime(&d.and_hms_opt(0, 0, 0)?));
        }
    }
    let b = raw.as_bytes();
    let n = b.len();
    if n > 5
        && (b[n - 5] == b'+' || b[n - 5] == b'-')
        && b[n - 4..].iter().all(|c| c.is_ascii_digit())
    {
        let mut fixed = String::from(&raw[..n - 2]);
        fixed.push(':');
        fixed.push_str(&raw[n - 2..]);
        if let Ok(dt) = DateTime::parse_from_rfc3339(&fixed) {
            return Some(dt.with_timezone(&Utc));
        }
    }
    None
}

pub fn time_splits_of(blocks: &[ValueBlock]) -> Vec<&ValueTimeSplit> {
    blocks.iter().flat_map(|b| b.time_splits.iter()).collect()
}

/// "Season 4: The Pilot · Ch.3". Words come from the caller so this stays free of i18n.
pub fn format_numbers_badge(season_word: &str, episode_word: &str, numbers: &ResolvedNumbers) -> Option<String> {
    if numbers.is_empty() {
        return None;
    }
    let mut parts = Vec::new();
    if let Some(num) = numbers.season_number {
        if let Some(name) = &numbers.season_name {
            parts.push(format!("{season_word} {num}: {name}"));
        } else {
            parts.push(format!("{season_word} {num}"));
        }
    } else if let Some(name) = &numbers.season_name {
        parts.push(name.clone());
    }
    if let Some(text) = &numbers.episode_text {
        let numeric = text.chars().all(|c| c.is_ascii_digit());
        if numeric {
            parts.push(format!("{episode_word} {text}"));
        } else {
            parts.push(text.clone());
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" · "))
    }
}

/// Trailers grouped by their season attribute, in the order seasons first appear.
pub fn trailer_groups(trailers: &[Trailer]) -> Vec<(Option<String>, Vec<usize>)> {
    let mut groups: Vec<(Option<String>, Vec<usize>)> = Vec::new();
    for (index, trailer) in trailers.iter().enumerate() {
        let key = trailer
            .season
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        if let Some(group) = groups.iter_mut().find(|(season, _)| season == &key) {
            group.1.push(index);
        } else {
            groups.push((key, vec![index]));
        }
    }
    groups
}

fn srcset_of(set: &ImageSet) -> Option<String> {
    if !set.sources.is_empty() {
        let parts: Vec<String> = set
            .sources
            .iter()
            .filter(|s| http_link(&s.url).is_some())
            .map(|s| match s.descriptor.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
                Some(d) => format!("{} {d}", s.url.trim()),
                None => s.url.trim().to_string(),
            })
            .collect();
        if parts.is_empty() {
            None
        } else {
            Some(parts.join(", "))
        }
    } else if set.srcset.trim().is_empty() {
        None
    } else {
        let parts: Vec<String> = set
            .srcset
            .split(',')
            .filter_map(|part| {
                let part = part.trim();
                let url = part.split_whitespace().next().unwrap_or("");
                if http_link(url).is_some() {
                    Some(part.to_string())
                } else {
                    None
                }
            })
            .collect();
        if parts.is_empty() {
            None
        } else {
            Some(parts.join(", "))
        }
    }
}

fn largest_srcset_url(srcset: &str) -> Option<String> {
    let mut best: Option<(u32, String)> = None;
    for part in srcset.split(',') {
        let mut bits = part.split_whitespace();
        let url = bits.next()?.trim().to_string();
        let width = bits
            .next()
            .and_then(|d| d.trim_end_matches(['w', 'W']).parse::<u32>().ok())
            .unwrap_or(0);
        if best.as_ref().map(|(w, _)| width >= *w).unwrap_or(true) {
            best = Some((width, url));
        }
    }
    best.map(|(_, url)| url)
}

fn parse_geo(raw: &str) -> Option<(String, String)> {
    let raw = raw.trim().trim_start_matches("geo:").trim_start_matches("GEO:");
    let mut parts = raw.split(',');
    let lat: f64 = parts.next()?.trim().parse().ok()?;
    let lon: f64 = parts.next()?.trim().parse().ok()?;
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return None;
    }
    Some((trim_coord(lat), trim_coord(lon)))
}

fn trim_coord(n: f64) -> String {
    let s = format!("{n:.6}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn osm_url(raw: &str) -> Option<String> {
    let raw = raw.trim();
    let (kind, rest) = raw.split_at_checked(1)?;
    let id = rest.trim().parse::<u64>().ok()?;
    let path = match kind {
        "N" | "n" => "node",
        "W" | "w" => "way",
        "R" | "r" => "relation",
        _ => return None,
    };
    Some(format!("https://www.openstreetmap.org/{path}/{id}"))
}

fn parse_seconds(raw: &str) -> Option<f64> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    raw.parse::<f64>().ok().filter(|n| n.is_finite())
}

fn parse_whole(raw: &str) -> Option<i64> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if let Ok(n) = raw.parse::<i64>() {
        return Some(n).filter(|n| *n > 0);
    }
    raw.parse::<f64>()
        .ok()
        .filter(|n| n.is_finite() && *n > 0.0)
        .map(|n| n.floor() as i64)
}

fn non_empty(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

fn is_dangerous(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    lower.starts_with("javascript:") || lower.starts_with("data:") || lower.starts_with("vbscript:")
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    fn chapter(start: Option<i32>, img: Option<&str>) -> crate::requests::pod_req::Chapter {
        crate::requests::pod_req::Chapter {
            startTime: start,
            title: String::new(),
            url: None,
            img: img.map(|s| s.to_string()),
        }
    }

    #[wasm_bindgen_test]
    fn scrolling_the_transcript_stops_following_until_a_jump_or_resume() {
        assert!(!follows_after(TranscriptFollow::UserScrolled));
        assert!(follows_after(TranscriptFollow::Seeked));
        assert!(follows_after(TranscriptFollow::Resumed));
    }

    #[wasm_bindgen_test]
    fn srt_cues_parse_and_the_current_one_follows_the_position() {
        let srt = "1\n00:00:01,000 --> 00:00:04,500\nHello\n\n2\n00:00:06,000 --> 00:00:09,000\n<v Adam>Second</v> line\n\n";
        let cues = parse_transcript_cues(srt);
        assert_eq!(cues.len(), 2);
        assert_eq!((cues[0].start, cues[0].end, cues[0].text.as_str()), (1.0, 4.5, "Hello"));
        assert_eq!(cues[1].text, "Second line");
        assert_eq!(parse_ts("01:02:03.5"), Some(3723.5));
        assert!(parse_transcript_cues("plain text, no timings").is_empty());
        assert_eq!(current_cue(&cues, 0.5), None);
        assert_eq!(current_cue(&cues, 2.0), Some(0));
        // Held through the gap until the next cue starts.
        assert_eq!(current_cue(&cues, 5.0), Some(0));
        assert_eq!(current_cue(&cues, 6.0), Some(1));
        assert_eq!(current_cue(&cues, 600.0), Some(1));
        assert_eq!(current_cue(&cues, f64::NAN), None);
    }

    #[wasm_bindgen_test]
    fn chapters_end_where_the_next_starts_and_follow_the_position() {
        let chapters = vec![
            chapter(Some(0), Some("intro.jpg")),
            chapter(Some(60), None),
            chapter(Some(300), Some("news.jpg")),
        ];
        assert_eq!(
            chapter_bounds(&chapters, 900.0),
            vec![(0.0, 60.0), (60.0, 300.0), (300.0, 900.0)]
        );
        assert_eq!(chapter_progress(30.0, 0.0, 60.0), 50.0);
        assert_eq!(chapter_progress(60.0, 0.0, 60.0), 100.0);
        assert_eq!(chapter_progress(30.0, 60.0, 300.0), 0.0);
        assert_eq!(chapter_progress(10.0, 10.0, 10.0), 100.0);
        assert_eq!(chapter_image_at(&chapters, 30.0), Some("intro.jpg"));
        // A chapter without art shows the episode artwork, not the last image.
        assert_eq!(chapter_image_at(&chapters, 120.0), None);
        assert_eq!(chapter_image_at(&chapters, 600.0), Some("news.jpg"));
        assert_eq!(chapter_image_at(&[chapter(None, Some("x.jpg"))], 30.0), None);
    }

    fn season(number: &str, name: Option<&str>) -> Season {
        Season {
            number: number.to_string(),
            name: name.map(|s| s.to_string()),
        }
    }

    fn episode(number: &str, display: Option<&str>) -> EpisodeNumber {
        EpisodeNumber {
            number: number.to_string(),
            display: display.map(|s| s.to_string()),
        }
    }

    #[test]
    #[wasm_bindgen_test]
    fn podcasting2_numbers_beat_itunes() {
        let resolved = resolve_numbers(
            Some(&season("4", Some("The Pilot"))),
            Some(&episode("12", Some("Ch.3"))),
            Some(9),
            Some(4),
        );
        assert_eq!(resolved.season_number, Some(4));
        assert_eq!(resolved.season_name.as_deref(), Some("The Pilot"));
        assert_eq!(resolved.episode_number, Some(12));
        assert_eq!(resolved.episode_text.as_deref(), Some("Ch.3"));
        assert!(resolved.from_podcasting2);
    }

    #[test]
    #[wasm_bindgen_test]
    fn itunes_fills_a_missing_podcasting2_number() {
        let resolved = resolve_numbers(None, None, Some(2), Some(8));
        assert_eq!(resolved.season_number, Some(2));
        assert_eq!(resolved.episode_number, Some(8));
        assert_eq!(resolved.episode_text.as_deref(), Some("8"));
        assert!(!resolved.from_podcasting2);

        let only_episode = resolve_numbers(None, Some(&episode("3", None)), Some(9), Some(1));
        assert_eq!(only_episode.season_number, Some(9));
        assert_eq!(only_episode.episode_number, Some(3));
    }

    #[test]
    #[wasm_bindgen_test]
    fn season_sort_puts_numbered_episodes_in_order() {
        let a = resolve_numbers(Some(&season("2", None)), Some(&episode("1", None)), None, None);
        let b = resolve_numbers(Some(&season("1", None)), Some(&episode("10", None)), None, None);
        let c = resolve_numbers(None, None, None, None);
        let mut rows = vec![c.clone(), a.clone(), b.clone()];
        rows.sort_by_key(season_sort_key);
        assert_eq!(rows[0].season_number, Some(1));
        assert_eq!(rows[1].season_number, Some(2));
        assert!(rows[2].is_empty());
        assert!(should_group_by_season(&[a, b]));
        assert!(!should_group_by_season(&[c]));
    }

    #[test]
    #[wasm_bindgen_test]
    fn soundbite_window_and_clock() {
        let clip = soundbite_window("73.0", "60.0").unwrap();
        assert_eq!(clip.start, 73.0);
        assert_eq!(clip.end, 133.0);
        assert_eq!(format_clock(73.0), "1:13");
        assert_eq!(format_clock(3661.0), "1:01:01");
        assert!(soundbite_window("nope", "1").is_none());
    }

    #[test]
    #[wasm_bindgen_test]
    fn location_links_prefer_geo_then_osm() {
        let both = Location {
            name: "Austin, TX".into(),
            rel: None,
            geo: Some("geo:30.2672,-97.7431".into()),
            osm: Some("R113314".into()),
            country: Some("US".into()),
        };
        assert_eq!(
            location_map_url(&both).as_deref(),
            Some("https://www.openstreetmap.org/?mlat=30.2672&mlon=-97.7431#map=14/30.2672/-97.7431")
        );
        let osm_only = Location {
            name: "Node".into(),
            rel: None,
            geo: None,
            osm: Some("N123".into()),
            country: None,
        };
        assert_eq!(
            location_map_url(&osm_only).as_deref(),
            Some("https://www.openstreetmap.org/node/123")
        );
        assert!(location_map_url(&Location {
            name: "Nowhere".into(),
            rel: None,
            geo: Some("not-geo".into()),
            osm: None,
            country: None,
        })
        .is_none());
    }

    #[test]
    #[wasm_bindgen_test]
    fn chat_and_social_links() {
        let irc = Chat {
            server: "irc.example.invalid".into(),
            protocol: "irc".into(),
            account_id: Some("@ada".into()),
            space: Some("#show".into()),
            embed_url: None,
        };
        assert_eq!(chat_url(&irc).as_deref(), Some("irc://irc.example.invalid/show"));
        let kiwi = Chat {
            embed_url: Some("https://kiwiirc.example.invalid/nextclient/".into()),
            ..irc.clone()
        };
        assert_eq!(
            chat_url(&kiwi).as_deref(),
            Some("https://kiwiirc.example.invalid/nextclient/")
        );
        let unsafe_embed = Chat {
            embed_url: Some("javascript:alert(1)".into()),
            ..irc.clone()
        };
        assert_eq!(chat_url(&unsafe_embed).as_deref(), Some("irc://irc.example.invalid/show"));
        let matrix = Chat {
            server: "matrix.example.invalid".into(),
            protocol: "matrix".into(),
            account_id: None,
            space: Some("#ep".into()),
            embed_url: None,
        };
        assert_eq!(
            chat_url(&matrix).as_deref(),
            Some("https://matrix.to/#/%23ep%3Amatrix.example.invalid")
        );
        let disabled = SocialInteract {
            protocol: "disabled".into(),
            uri: None,
            account_id: None,
            account_url: None,
            priority: None,
        };
        assert!(social_url(&disabled).is_none());
        let thread = SocialInteract {
            protocol: "activitypub".into(),
            uri: Some("https://example.invalid/ap/ep".into()),
            account_id: None,
            account_url: None,
            priority: Some("2".into()),
        };
        assert_eq!(social_url(&thread).as_deref(), Some("https://example.invalid/ap/ep"));
        assert!(safe_link("javascript:alert(1)").is_none());
        assert!(safe_link("ipfs://bafy").is_some());
    }

    #[test]
    #[wasm_bindgen_test]
    fn artwork_uses_srcset_and_drops_unsafe_urls() {
        let images = vec![PodcastImage {
            href: "https://cdn.example.invalid/art.jpg".into(),
            alt: Some("Cover".into()),
            aspect_ratio: None,
            width: Some("1400".into()),
            height: None,
            purpose: Some("artwork".into()),
        }];
        let set = ImageSet {
            srcset: "javascript:alert(1) 10w, https://cdn.example.invalid/img/small.jpg 300w, https://cdn.example.invalid/img/big.jpg 1500w".into(),
            sources: vec![],
        };
        let choice = artwork_choice(&images, Some(&set), "https://fallback.example/a.jpg");
        assert_eq!(choice.src, "https://cdn.example.invalid/art.jpg");
        assert_eq!(choice.alt.as_deref(), Some("Cover"));
        assert_eq!(
            choice.srcset.as_deref(),
            Some("https://cdn.example.invalid/img/small.jpg 300w, https://cdn.example.invalid/img/big.jpg 1500w")
        );
        let from_set = artwork_choice(&[], Some(&set), "https://fallback.example/a.jpg");
        assert_eq!(from_set.src, "https://cdn.example.invalid/img/big.jpg");
    }

    #[test]
    #[wasm_bindgen_test]
    fn schedule_and_time_splits() {
        assert_eq!(cadence_of("FREQ=WEEKLY;INTERVAL=2"), Some(Cadence::Weekly(2)));
        assert_eq!(cadence_of("INTERVAL=3;FREQ=DAILY"), Some(Cadence::Daily(3)));
        assert!(cadence_of("FREQ=SECONDLY").is_none());
        let block = ValueBlock {
            kind: "lightning".into(),
            method: "keysend".into(),
            suggested: None,
            recipients: vec![],
            time_splits: vec![ValueTimeSplit {
                start_time: "63".into(),
                duration: "388".into(),
                remote_start_time: None,
                remote_percentage: Some("40".into()),
                recipients: vec![ValueRecipient {
                    name: "Guest Split".into(),
                    kind: "node".into(),
                    address: "02ff".into(),
                    split: "50".into(),
                }],
                remote_item: None,
            }],
        };
        let blocks = vec![block];
        let splits = time_splits_of(&blocks);
        assert_eq!(splits.len(), 1);
        assert_eq!(splits[0].recipients[0].name, "Guest Split");
        assert!(trailer_is_video(Some("video/mp4")));
        assert!(!trailer_is_video(Some("audio/mpeg")));
        let labeled = resolve_numbers(
            Some(&season("4", Some("The Pilot"))),
            Some(&episode("12", Some("Ch.3"))),
            None,
            None,
        );
        assert_eq!(
            format_numbers_badge("Season", "Episode", &labeled).as_deref(),
            Some("Season 4: The Pilot · Ch.3")
        );
        let trailers = vec![
            Trailer {
                title: "A".into(),
                url: "https://example.invalid/a.mp3".into(),
                pubdate: String::new(),
                length: None,
                mime_type: None,
                season: None,
            },
            Trailer {
                title: "B".into(),
                url: "https://example.invalid/b.mp4".into(),
                pubdate: String::new(),
                length: None,
                mime_type: Some("video/mp4".into()),
                season: Some("4".into()),
            },
        ];
        let groups = trailer_groups(&trailers);
        assert_eq!(groups.len(), 2);
        assert!(groups[0].0.is_none());
        assert_eq!(groups[1].0.as_deref(), Some("4"));
    }

    #[test]
    #[wasm_bindgen_test]
    fn live_items_play_hls_and_hide_ended_recordings() {
        let raw = r##"{
            "complete": true,
            "live_items": [
                {"status": "ended", "start": "2026-01-02T00:00:00Z", "tags": {"guid": "kept", "enclosure_url": "https://cdn.example/rec.mp3"}},
                {"status": "ended", "start": "2026-01-03T00:00:00Z", "title": "Unmatched ended", "tags": {"guid": "other"}},
                {"status": "live", "start": "2020-01-01T00:00:00Z", "end": "2020-01-01T01:00:00Z", "title": "Past end", "tags": {"guid": "past"}},
                {"status": "pending", "start": "2099-03-01T00:00:00Z", "end": "2099-03-01T02:00:00+0000", "title": "Soon", "tags": {"guid": "soon"}},
                {"status": "live", "start": "2099-02-01T00:00:00Z", "title": "On air", "enclosure_type": "application/vnd.apple.mpegurl", "live_value": {"uri": "https://live.example.invalid/socket.io/", "protocol": "socket.io"}, "tags": {"guid": "now", "enclosure_url": "https://cdn.example/live"}}
            ]
        }"##;
        let tags: ChannelTags = serde_json::from_str(raw).unwrap();
        assert_eq!(tags.complete, Some(true));
        let visible = visible_live_items(&tags.live_items, &["live-1".into()]);
        assert_eq!(visible.len(), 2);
        assert_eq!(normalize_live_status(&visible[0].status), "live");
        assert_eq!(visible[0].title.as_deref(), Some("On air"));
        assert_eq!(
            visible[0].live_value.as_ref().map(|v| v.protocol.as_str()),
            Some("socket.io")
        );
        assert_eq!(normalize_live_status(&visible[1].status), "pending");
        let playback = live_playback(
            visible[0].enclosure_type.as_deref(),
            visible[0].tags.enclosure_url.as_deref().unwrap(),
        )
        .unwrap();
        assert!(playback.hls);
        assert!(!playback.is_video);
        assert!(live_playback(Some("audio/mpeg"), "javascript:alert(1)").is_none());
        assert!(live_stream_is_video(Some("video/mp4")));
    }

    #[test]
    #[wasm_bindgen_test]
    fn sample_episode_tags_deserialize() {
        let raw = r##"{
            "soundbites": [{"start_time": "73.0", "duration": "60.0"}, {"start_time": "1234.5", "duration": "42.25", "title": "Rock & Roll"}],
            "season": {"number": "4", "name": "The Pilot"},
            "episode": {"number": "12", "display": "Ch.3"},
            "content_links": [{"href": "https://example.invalid/watch", "text": "Watch this episode"}],
            "social_interacts": [{"protocol": "disabled"}, {"protocol": "activitypub", "uri": "https://example.invalid/ap/ep", "priority": "2"}],
            "chat": {"server": "matrix.example.invalid", "protocol": "matrix", "space": "#ep"},
            "remote_items": [{"feed_guid": "b40ffcf7-2c48-5cfe-8daa-b65d766b2c25", "item_guid": "other-item", "medium": "podcast"}],
            "value": [{"type": "lightning", "method": "keysend", "suggested": "0.00000001000", "recipients": [], "time_splits": [{"start_time": "63", "duration": "388", "remote_percentage": "40", "recipients": [{"name": "Guest Split", "type": "node", "address": "02ff", "split": "50"}]}]}],
            "locations": [{"name": "New York", "geo": "geo:40.7,-74.0", "osm": "N123", "country": "US"}]
        }"##;
        let tags: ItemTags = serde_json::from_str(raw).unwrap();
        assert_eq!(tags.soundbites.len(), 2);
        assert_eq!(tags.season.unwrap().name.as_deref(), Some("The Pilot"));
        assert_eq!(tags.episode.unwrap().display.as_deref(), Some("Ch.3"));
        assert_eq!(social_url(&tags.social_interacts[0]), None);
        assert!(social_url(&tags.social_interacts[1]).is_some());
        assert_eq!(tags.value[0].time_splits[0].recipients[0].name, "Guest Split");
        assert!(location_map_url(&tags.locations[0]).is_some());

        let wrapped = r#"{"chapters": [], "transcripts": [], "people": [], "tags": {"medium": "music"}}"#;
        #[derive(Deserialize)]
        struct Wrap {
            #[serde(default)]
            tags: ItemTags,
        }
        let wrap: Wrap = serde_json::from_str(wrapped).unwrap();
        assert_eq!(wrap.tags.medium.as_deref(), Some("music"));
        let legacy = r#"{"chapters": [], "transcripts": [], "people": []}"#;
        let legacy: Wrap = serde_json::from_str(legacy).unwrap();
        assert!(legacy.tags.soundbites.is_empty());
    }
}
