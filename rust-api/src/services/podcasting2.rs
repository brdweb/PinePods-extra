//! Podcasting 2.0 tags, read once per feed at refresh time and stored, so podcast and episode
//! pages never fetch a feed to show them.
//!
//! One streaming pass with quick-xml. Tag order, line layout, namespace prefix and escaping don't
//! matter. A malformed tail still yields everything parsed before it. Only elements in the
//! Podcasting 2.0 namespace are read, so `itunes:*` tags with the same local names are ignored.
//! The two namespace URIs in the spec (`https://podcastindex.org/namespace/1.0` and the canonical
//! `docs/1.0.md` URL) are treated as the same namespace.
//!
//! `podcast:podping` is stored on the show. The optional Hive listener that refreshes a
//! subscribed feed when Podping names it lives in `podping` and is off unless configured.

use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::{Namespace, ResolveResult};
use quick_xml::reader::NsReader;
use serde::{Deserialize, Serialize};

const NS: &str = "https://podcastindex.org/namespace/1.0";
const NS_ALT: &str = "https://github.com/Podcastindex-org/podcast-namespace/blob/main/docs/1.0.md";

/// Minimum gap the scheduler will honor, and the longest it will wait before checking again.
const MIN_REFRESH_MINUTES: i64 = 60;
const MAX_REFRESH_MINUTES: i64 = 30 * 24 * 60;
const COMPLETE_REFRESH_MINUTES: i64 = 7 * 24 * 60;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
pub struct FeedPerson {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub img: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Transcript {
    pub url: String,
    pub mime_type: Option<String>,
    pub language: Option<String>,
    pub rel: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Soundbite {
    pub start_time: String,
    pub duration: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Season {
    pub number: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
pub struct EpisodeNumber {
    pub number: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
pub struct License {
    pub identifier: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Source {
    pub uri: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Integrity {
    #[serde(rename = "type")]
    pub kind: String,
    pub value: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AlternateEnclosure {
    pub mime_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bitrate: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rel: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codecs: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<Source>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integrity: Option<Integrity>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
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
    pub mime_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ImageSource {
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub descriptor: Option<String>,
}

/// Deprecated `podcast:images` srcset. Prefer [`PodcastImage`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ImageSet {
    pub srcset: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<ImageSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ContentLink {
    pub href: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SocialInteract {
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Block {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Txt {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Chat {
    pub server: String,
    pub protocol: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub space: Option<String>,
    /// Web client for the room, for example a KiwiIRC link next to an `irc://` server.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embed_url: Option<String>,
}

/// `podcast:liveValue` on a live item: where the live split is published, plus any
/// nested `podcast:value` block. Stored only. Nothing here sends a payment.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
pub struct LiveValue {
    pub uri: String,
    pub protocol: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub value: Vec<ValueBlock>,
}

/// A `podcast:remoteItem`. `feed_guid` is the spec's required `feedGuid`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RemoteItem {
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

/// Historical name for a podroll entry. Podroll entries are remote items.
pub type PodrollItem = RemoteItem;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ValueRecipient {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub address: String,
    pub split: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fee: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ValueTimeSplit {
    pub start_time: String,
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

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ValueBlock {
    #[serde(rename = "type")]
    pub kind: String,
    pub method: String,
    pub suggested: Option<String>,
    pub recipients: Vec<ValueRecipient>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub time_splits: Vec<ValueTimeSplit>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ItemTags {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enclosure_url: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub people: Vec<FeedPerson>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chapters_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chapters_type: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transcripts: Vec<Transcript>,
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
    pub alternate_enclosures: Vec<AlternateEnclosure>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub image: Vec<PodcastImage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub images: Option<ImageSet>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content_links: Vec<ContentLink>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub social_interacts: Vec<SocialInteract>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<Block>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub txts: Vec<Txt>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub funding: Vec<Funding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat: Option<Chat>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub value: Vec<ValueBlock>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remote_items: Vec<RemoteItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Funding {
    pub url: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Locked {
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Trailer {
    pub title: String,
    pub url: String,
    pub pubdate: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub season: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
pub struct UpdateFrequency {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub complete: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dtstart: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rrule: Option<String>,
}

/// Present when the feed includes `podcast:podping`. This records the tag. The optional
/// Hive listener is separate and matches the stored feed URL.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Podping {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uses_podping: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
pub struct LiveItem {
    pub status: String,
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
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pub_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enclosure_length: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enclosure_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub live_value: Option<LiveValue>,
    #[serde(default)]
    pub tags: ItemTags,
}

/// Channel-level data stored as JSON on the podcast row (`Podcasts.Podcasting2Data`).
///
/// New fields deserialize as empty when missing, so JSON written before migration 063 still loads.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ChannelExtras {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guid: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub podroll: Vec<RemoteItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub funding: Vec<Funding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub value: Vec<ValueBlock>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locked: Option<Locked>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub locations: Vec<Location>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trailers: Vec<Trailer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<License>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub medium: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub image: Vec<PodcastImage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub images: Option<ImageSet>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub live_items: Vec<LiveItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub social_interacts: Vec<SocialInteract>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<Block>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub txts: Vec<Txt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_frequency: Option<UpdateFrequency>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub podping: Option<Podping>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher: Option<RemoteItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat: Option<Chat>,
    /// Channel `podcast:complete`. Presence of the tag means the show will not add episodes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub complete: Option<bool>,
    /// `podcast:remoteItem` children of `<channel>` (list feeds), not podroll or publisher.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remote_items: Vec<RemoteItem>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct FeedTags {
    pub podcast_guid: Option<String>,
    pub people: Vec<FeedPerson>,
    pub extras: ChannelExtras,
    /// Only items carrying at least one tag we store.
    pub items: Vec<ItemTags>,
}

impl FeedTags {
    /// Stable fingerprint, stored with the podcast so an unchanged feed costs no writes.
    pub fn fingerprint(&self) -> String {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.hash(&mut h);
        format!("{:016x}", h.finish())
    }

    pub fn has_people(&self) -> bool {
        !self.people.is_empty() || self.items.iter().any(|i| !i.people.is_empty())
    }
}

/// When the scheduler should next fetch a feed that declared `podcast:updateFrequency`.
///
/// `interval_minutes` is the ongoing gap. `next_at` is the first moment the feed is due again.
/// Both are `None` when the tag is absent or has no machine-readable schedule, which leaves the
/// global refresh cron in charge. A `complete` feed is rechecked weekly. Recurrence is never
/// polled faster than hourly and never left unchecked for more than 30 days, so a yearly or
/// far-future `dtstart` still gets a monthly look in case the feed changes its mind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshPlan {
    pub interval_minutes: Option<i32>,
    pub next_at: Option<DateTime<Utc>>,
}

pub fn refresh_plan(freq: &Option<UpdateFrequency>, now: DateTime<Utc>) -> RefreshPlan {
    let Some(freq) = freq else {
        return RefreshPlan {
            interval_minutes: None,
            next_at: None,
        };
    };
    if freq.complete == Some(true) {
        return RefreshPlan {
            interval_minutes: Some(COMPLETE_REFRESH_MINUTES as i32),
            next_at: Some(now + Duration::minutes(COMPLETE_REFRESH_MINUTES)),
        };
    }
    let interval = freq.rrule.as_deref().and_then(rrule_minutes);
    let mut next = interval.map(|m| now + Duration::minutes(i64::from(m)));
    if let Some(start) = freq.dtstart.as_deref().and_then(parse_dtstart) {
        if start > now {
            let cap = now + Duration::minutes(MAX_REFRESH_MINUTES);
            next = Some(if start < cap { start } else { cap });
        }
    }
    RefreshPlan {
        interval_minutes: interval,
        next_at: next,
    }
}

fn rrule_minutes(rrule: &str) -> Option<i32> {
    let mut freq = String::new();
    let mut interval: i64 = 1;
    for part in rrule.split(';') {
        let Some((k, v)) = part.split_once('=') else {
            continue;
        };
        match k.trim().to_ascii_uppercase().as_str() {
            "FREQ" => freq = v.trim().to_ascii_uppercase(),
            "INTERVAL" => interval = v.trim().parse::<i64>().unwrap_or(1).max(1),
            _ => {}
        }
    }
    let base: i64 = match freq.as_str() {
        "HOURLY" => 60,
        "DAILY" => 24 * 60,
        "WEEKLY" => 7 * 24 * 60,
        "MONTHLY" => 30 * 24 * 60,
        "YEARLY" => 365 * 24 * 60,
        _ => return None,
    };
    Some(
        base.saturating_mul(interval)
            .clamp(MIN_REFRESH_MINUTES, MAX_REFRESH_MINUTES) as i32,
    )
}

fn parse_dtstart(raw: &str) -> Option<DateTime<Utc>> {
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

fn attr(e: &BytesStart, key: &str) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == key)
        .and_then(|a| {
            a.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .ok()
                .map(|v| v.trim().to_string())
        })
        .filter(|v| !v.is_empty())
}

fn strict_bool(e: &BytesStart, key: &str) -> Option<bool> {
    match attr(e, key)
        .as_deref()
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("true") => Some(true),
        Some("false") => Some(false),
        _ => None,
    }
}

fn http_url(raw: Option<String>) -> Option<String> {
    let raw = raw?;
    let lower = raw.to_ascii_lowercase();
    (lower.starts_with("http://") || lower.starts_with("https://")).then_some(raw)
}

/// Any URI except scriptable schemes. `ipfs:` and similar transports stay.
fn safe_uri(raw: Option<String>) -> Option<String> {
    let raw = raw?;
    let lower = raw.trim().to_ascii_lowercase();
    if lower.starts_with("javascript:")
        || lower.starts_with("data:")
        || lower.starts_with("vbscript:")
    {
        return None;
    }
    Some(raw)
}

fn local_name<'a>(e: &'a BytesStart) -> &'a str {
    e.local_name().into_inner()
}

fn is_podcast(ns: &ResolveResult) -> bool {
    match ns {
        ResolveResult::Bound(Namespace(uri)) => *uri == NS || *uri == NS_ALT,
        _ => false,
    }
}

fn clean_name(raw: &str) -> String {
    raw.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(200)
        .collect()
}

fn finish_text(raw: String) -> String {
    let v = quick_xml::escape::unescape(&raw)
        .map(|c| c.into_owned())
        .unwrap_or(raw);
    v.trim().to_string()
}

fn push_text(buf: &mut String, extra: &str) {
    const CAP: usize = 16_000;
    let room = CAP.saturating_sub(buf.len());
    if room == 0 || extra.is_empty() {
        return;
    }
    let mut end = extra.len().min(room);
    while end > 0 && !extra.is_char_boundary(end) {
        end -= 1;
    }
    buf.push_str(&extra[..end]);
}

fn parse_srcset(raw: &str) -> ImageSet {
    let mut sources = Vec::new();
    for part in raw.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let mut bits = part.split_whitespace();
        let Some(url) = http_url(bits.next().map(str::to_string)) else {
            continue;
        };
        let descriptor = bits.next().map(str::to_string);
        sources.push(ImageSource { url, descriptor });
    }
    let srcset = sources
        .iter()
        .map(|s| match &s.descriptor {
            Some(d) => format!("{} {}", s.url, d),
            None => s.url.clone(),
        })
        .collect::<Vec<_>>()
        .join(", ");
    ImageSet { srcset, sources }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CapKind {
    Person,
    Funding,
    Location,
    Locked,
    Trailer,
    License,
    Season,
    Episode,
    ContentLink,
    Txt,
    UpdateFrequency,
    Medium,
    Soundbite,
    Block,
    PodcastGuid,
    ItemGuid,
    Title,
    Description,
    Link,
    Author,
    PubDate,
    Complete,
}

fn cap_name(kind: CapKind) -> &'static str {
    match kind {
        CapKind::Person => "person",
        CapKind::Funding => "funding",
        CapKind::Location => "location",
        CapKind::Locked => "locked",
        CapKind::Trailer => "trailer",
        CapKind::License => "license",
        CapKind::Season => "season",
        CapKind::Episode => "episode",
        CapKind::ContentLink => "contentLink",
        CapKind::Txt => "txt",
        CapKind::UpdateFrequency => "updateFrequency",
        CapKind::Medium => "medium",
        CapKind::Soundbite => "soundbite",
        CapKind::Block => "block",
        CapKind::PodcastGuid | CapKind::ItemGuid => "guid",
        CapKind::Title => "title",
        CapKind::Description => "description",
        CapKind::Link => "link",
        CapKind::Author => "author",
        CapKind::PubDate => "pubDate",
        CapKind::Complete => "complete",
    }
}

enum Frame {
    Item(ItemTags),
    Live(LiveItem),
    LiveValue(LiveValue),
    Podroll,
    Publisher,
    Value(ValueBlock),
    Alt(AlternateEnclosure),
    Split(ValueTimeSplit),
}

struct Capture {
    kind: CapKind,
    el: BytesStart<'static>,
    depth: u32,
    buf: String,
}

fn blocked(stack: &[Frame]) -> bool {
    matches!(
        stack.last(),
        Some(
            Frame::Value(_)
                | Frame::Alt(_)
                | Frame::Split(_)
                | Frame::Podroll
                | Frame::Publisher
                | Frame::LiveValue(_)
        )
    )
}

fn direct_item(stack: &[Frame]) -> bool {
    matches!(stack.last(), Some(Frame::Item(_)))
}

fn direct_live(stack: &[Frame]) -> bool {
    matches!(stack.last(), Some(Frame::Live(_)))
}

fn text_kind(podcast: bool, local: &str, stack: &[Frame]) -> Option<CapKind> {
    if blocked(stack) {
        return None;
    }
    let in_item = direct_item(stack) || direct_live(stack);
    if podcast {
        return match local {
            "person" => Some(CapKind::Person),
            "funding" => Some(CapKind::Funding),
            "location" => Some(CapKind::Location),
            "locked" if !in_item => Some(CapKind::Locked),
            "trailer" if !in_item => Some(CapKind::Trailer),
            "license" => Some(CapKind::License),
            "season" if in_item => Some(CapKind::Season),
            "episode" if in_item => Some(CapKind::Episode),
            "contentLink" if in_item => Some(CapKind::ContentLink),
            "txt" => Some(CapKind::Txt),
            "updateFrequency" if !in_item => Some(CapKind::UpdateFrequency),
            "complete" if !in_item => Some(CapKind::Complete),
            "medium" if !in_item => Some(CapKind::Medium),
            "soundbite" if in_item => Some(CapKind::Soundbite),
            "block" => Some(CapKind::Block),
            "guid" if !in_item => Some(CapKind::PodcastGuid),
            _ => None,
        };
    }
    if direct_live(stack) {
        return match local {
            "title" => Some(CapKind::Title),
            "description" => Some(CapKind::Description),
            "link" => Some(CapKind::Link),
            "author" => Some(CapKind::Author),
            "guid" => Some(CapKind::ItemGuid),
            "pubDate" | "pubdate" => Some(CapKind::PubDate),
            _ => None,
        };
    }
    if direct_item(stack) && local == "guid" {
        Some(CapKind::ItemGuid)
    } else {
        None
    }
}

fn item_tags_mut(stack: &mut [Frame]) -> Option<&mut ItemTags> {
    match stack.last_mut() {
        Some(Frame::Item(it)) => Some(it),
        Some(Frame::Live(live)) => Some(&mut live.tags),
        _ => None,
    }
}

fn interesting(it: &ItemTags) -> bool {
    it.chapters_url.is_some()
        || it.season.is_some()
        || it.episode.is_some()
        || it.license.is_some()
        || it.images.is_some()
        || it.chat.is_some()
        || !it.people.is_empty()
        || !it.transcripts.is_empty()
        || !it.soundbites.is_empty()
        || !it.locations.is_empty()
        || !it.alternate_enclosures.is_empty()
        || !it.image.is_empty()
        || !it.content_links.is_empty()
        || !it.social_interacts.is_empty()
        || !it.blocks.is_empty()
        || !it.txts.is_empty()
        || !it.funding.is_empty()
        || !it.value.is_empty()
        || !it.remote_items.is_empty()
}

pub fn parse(feed: &str) -> FeedTags {
    let mut reader = NsReader::from_str(feed);
    let mut tags = FeedTags::default();
    let mut stack: Vec<Frame> = Vec::new();
    let mut capture: Option<Capture> = None;

    loop {
        let (ns, event) = match reader.read_resolved_event() {
            Ok((_, Event::Eof)) => {
                flush_open(&mut capture, &mut stack, &mut tags);
                break;
            }
            Ok(pair) => pair,
            Err(e) => {
                tracing::debug!("Podcasting 2.0 parse stopped early: {e}");
                flush_open(&mut capture, &mut stack, &mut tags);
                break;
            }
        };
        let podcast = is_podcast(&ns);
        match event {
            Event::Start(e) => {
                if let Some(cap) = capture.as_mut() {
                    cap.depth += 1;
                } else {
                    begin_element(e, podcast, false, &mut stack, &mut tags, &mut capture);
                }
            }
            Event::Empty(e) => {
                if capture.is_none() {
                    begin_element(e, podcast, true, &mut stack, &mut tags, &mut capture);
                }
            }
            Event::Text(t) => {
                if let Some(cap) = capture.as_mut() {
                    push_text(&mut cap.buf, &t.xml10_content());
                }
            }
            Event::GeneralRef(r) => {
                if let Some(cap) = capture.as_mut() {
                    let mut tmp = String::new();
                    tmp.push('&');
                    tmp.push_str(&r.xml10_content());
                    tmp.push(';');
                    push_text(&mut cap.buf, &tmp);
                }
            }
            Event::CData(c) => {
                if let Some(cap) = capture.as_mut() {
                    let escaped = quick_xml::escape::escape(c.into_inner().into_owned());
                    push_text(&mut cap.buf, &escaped);
                }
            }
            Event::End(e) => {
                let name = e.local_name().into_inner();
                if capture.as_ref().is_some_and(|cap| cap.depth > 1) {
                    capture.as_mut().unwrap().depth -= 1;
                    continue;
                }
                let matches_capture = capture.as_ref().is_some_and(|cap| {
                    name == cap_name(cap.kind)
                        || (cap.kind == CapKind::PubDate && name.eq_ignore_ascii_case("pubdate"))
                });
                if matches_capture {
                    let cap = capture.take().unwrap();
                    let text = finish_text(cap.buf);
                    commit_text(cap.kind, &cap.el, &text, &mut stack, &mut tags);
                    continue;
                }
                close_frame(name, podcast, &mut stack, &mut tags);
            }
            _ => {}
        }
    }
    tags
}

fn begin_element(
    e: BytesStart,
    podcast: bool,
    empty: bool,
    stack: &mut Vec<Frame>,
    tags: &mut FeedTags,
    capture: &mut Option<Capture>,
) {
    let local = local_name(&e).to_string();
    if let Some(kind) = text_kind(podcast, &local, stack) {
        if empty {
            commit_text(kind, &e, "", stack, tags);
        } else {
            *capture = Some(Capture {
                kind,
                el: e.into_owned(),
                depth: 1,
                buf: String::new(),
            });
        }
        return;
    }
    if try_open_frame(&e, podcast, stack) {
        if empty {
            close_frame(&local, podcast, stack, tags);
        }
        return;
    }
    handle_leaf(&e, podcast, stack, tags);
}

fn try_open_frame(e: &BytesStart, podcast: bool, stack: &mut Vec<Frame>) -> bool {
    let local = local_name(e);
    let in_item = direct_item(stack) || direct_live(stack);
    let blocked = blocked(stack);
    if podcast && local == "liveItem" && !in_item && !blocked {
        stack.push(Frame::Live(LiveItem {
            status: attr(e, "status").unwrap_or_default(),
            start: attr(e, "start").unwrap_or_default(),
            end: attr(e, "end"),
            ..LiveItem::default()
        }));
        return true;
    }
    if podcast && local == "liveValue" && direct_live(stack) {
        stack.push(Frame::LiveValue(LiveValue {
            uri: attr(e, "uri").unwrap_or_default(),
            protocol: attr(e, "protocol").unwrap_or_default(),
            ..LiveValue::default()
        }));
        return true;
    }
    if !podcast && local == "item" && !in_item && !blocked {
        stack.push(Frame::Item(ItemTags::default()));
        return true;
    }
    if podcast && local == "podroll" && stack.is_empty() {
        stack.push(Frame::Podroll);
        return true;
    }
    if podcast && local == "publisher" && stack.is_empty() {
        stack.push(Frame::Publisher);
        return true;
    }
    // A value block nested in liveValue is the live split. liveValue is otherwise
    // "blocked" so its text does not leak onto the live item.
    if podcast && local == "value" && (!blocked || matches!(stack.last(), Some(Frame::LiveValue(_)))) {
        stack.push(Frame::Value(ValueBlock {
            kind: attr(e, "type").unwrap_or_default(),
            method: attr(e, "method").unwrap_or_default(),
            suggested: attr(e, "suggested"),
            ..ValueBlock::default()
        }));
        return true;
    }
    if podcast && local == "alternateEnclosure" && in_item && !blocked {
        stack.push(Frame::Alt(AlternateEnclosure {
            mime_type: attr(e, "type").unwrap_or_default(),
            length: attr(e, "length"),
            bitrate: attr(e, "bitrate"),
            height: attr(e, "height"),
            lang: attr(e, "lang"),
            title: attr(e, "title"),
            rel: attr(e, "rel"),
            codecs: attr(e, "codecs"),
            default: strict_bool(e, "default"),
            ..AlternateEnclosure::default()
        }));
        return true;
    }
    if podcast && local == "valueTimeSplit" && matches!(stack.last(), Some(Frame::Value(_))) {
        stack.push(Frame::Split(ValueTimeSplit {
            start_time: attr(e, "startTime").unwrap_or_default(),
            duration: attr(e, "duration").unwrap_or_default(),
            remote_start_time: attr(e, "remoteStartTime"),
            remote_percentage: attr(e, "remotePercentage"),
            ..ValueTimeSplit::default()
        }));
        return true;
    }
    false
}

fn close_frame(name: &str, podcast: bool, stack: &mut Vec<Frame>, tags: &mut FeedTags) {
    let matches_top = match stack.last() {
        Some(Frame::Item(_)) => !podcast && name == "item",
        Some(Frame::Live(_)) => podcast && name == "liveItem",
        Some(Frame::LiveValue(_)) => podcast && name == "liveValue",
        Some(Frame::Podroll) => podcast && name == "podroll",
        Some(Frame::Publisher) => podcast && name == "publisher",
        Some(Frame::Value(_)) => podcast && name == "value",
        Some(Frame::Alt(_)) => podcast && name == "alternateEnclosure",
        Some(Frame::Split(_)) => podcast && name == "valueTimeSplit",
        None => false,
    };
    if !matches_top {
        return;
    }
    match stack.pop() {
        Some(Frame::Item(it)) => {
            if interesting(&it) {
                tags.items.push(it);
            }
        }
        Some(Frame::Live(live)) => tags.extras.live_items.push(live),
        Some(Frame::LiveValue(live_value)) => attach_live_value(stack, live_value),
        Some(Frame::Value(block)) => attach_value(stack, tags, block),
        Some(Frame::Alt(alt)) => attach_alt(stack, alt),
        Some(Frame::Split(split)) => attach_split(stack, split),
        _ => {}
    }
}

fn flush_open(capture: &mut Option<Capture>, stack: &mut Vec<Frame>, tags: &mut FeedTags) {
    if let Some(cap) = capture.take() {
        let text = finish_text(cap.buf);
        commit_text(cap.kind, &cap.el, &text, stack, tags);
    }
    while let Some(frame) = stack.pop() {
        match frame {
            Frame::Item(it) => {
                if interesting(&it) {
                    tags.items.push(it);
                }
            }
            Frame::Live(live) => tags.extras.live_items.push(live),
            Frame::LiveValue(live_value) => attach_live_value(stack, live_value),
            Frame::Value(block) => attach_value(stack, tags, block),
            Frame::Alt(alt) => attach_alt(stack, alt),
            Frame::Split(split) => attach_split(stack, split),
            Frame::Podroll | Frame::Publisher => {}
        }
    }
}

fn attach_value(stack: &mut [Frame], tags: &mut FeedTags, block: ValueBlock) {
    if block.recipients.is_empty() && block.time_splits.is_empty() {
        return;
    }
    match stack.last_mut() {
        Some(Frame::Item(it)) => it.value.push(block),
        Some(Frame::Live(live)) => live.tags.value.push(block),
        Some(Frame::LiveValue(live)) => live.value.push(block),
        _ => tags.extras.value.push(block),
    }
}

fn attach_live_value(stack: &mut [Frame], live_value: LiveValue) {
    if live_value.uri.is_empty() && live_value.protocol.is_empty() && live_value.value.is_empty() {
        return;
    }
    if let Some(Frame::Live(live)) = stack.last_mut() {
        if live.live_value.is_none() {
            live.live_value = Some(live_value);
        }
    }
}

fn attach_alt(stack: &mut [Frame], alt: AlternateEnclosure) {
    match stack.last_mut() {
        Some(Frame::Item(it)) => it.alternate_enclosures.push(alt),
        Some(Frame::Live(live)) => live.tags.alternate_enclosures.push(alt),
        _ => {}
    }
}

fn attach_split(stack: &mut [Frame], split: ValueTimeSplit) {
    if let Some(Frame::Value(v)) = stack.last_mut() {
        v.time_splits.push(split);
    }
}

fn push_person(stack: &mut [Frame], tags: &mut FeedTags, person: FeedPerson) {
    let list = match stack.last_mut() {
        Some(Frame::Item(it)) => &mut it.people,
        Some(Frame::Live(live)) => &mut live.tags.people,
        _ => &mut tags.people,
    };
    if !list.contains(&person) {
        list.push(person);
    }
}

fn push_funding(stack: &mut [Frame], tags: &mut FeedTags, funding: Funding) {
    match stack.last_mut() {
        Some(Frame::Item(it)) => it.funding.push(funding),
        Some(Frame::Live(live)) => live.tags.funding.push(funding),
        _ => tags.extras.funding.push(funding),
    }
}

fn push_location(stack: &mut [Frame], tags: &mut FeedTags, location: Location) {
    match stack.last_mut() {
        Some(Frame::Item(it)) => it.locations.push(location),
        Some(Frame::Live(live)) => live.tags.locations.push(location),
        _ => tags.extras.locations.push(location),
    }
}

fn push_block(stack: &mut [Frame], tags: &mut FeedTags, block: Block) {
    match stack.last_mut() {
        Some(Frame::Item(it)) => it.blocks.push(block),
        Some(Frame::Live(live)) => live.tags.blocks.push(block),
        _ => tags.extras.blocks.push(block),
    }
}

fn push_txt(stack: &mut [Frame], tags: &mut FeedTags, txt: Txt) {
    match stack.last_mut() {
        Some(Frame::Item(it)) => it.txts.push(txt),
        Some(Frame::Live(live)) => live.tags.txts.push(txt),
        _ => tags.extras.txts.push(txt),
    }
}

fn push_social(stack: &mut [Frame], tags: &mut FeedTags, social: SocialInteract) {
    match stack.last_mut() {
        Some(Frame::Item(it)) => it.social_interacts.push(social),
        Some(Frame::Live(live)) => live.tags.social_interacts.push(social),
        _ => tags.extras.social_interacts.push(social),
    }
}

fn push_image(stack: &mut [Frame], tags: &mut FeedTags, image: PodcastImage) {
    match stack.last_mut() {
        Some(Frame::Item(it)) => it.image.push(image),
        Some(Frame::Live(live)) => live.tags.image.push(image),
        _ => tags.extras.image.push(image),
    }
}

fn set_images(stack: &mut [Frame], tags: &mut FeedTags, images: ImageSet) {
    match stack.last_mut() {
        Some(Frame::Item(it)) => {
            if it.images.is_none() {
                it.images = Some(images);
            }
        }
        Some(Frame::Live(live)) => {
            if live.tags.images.is_none() {
                live.tags.images = Some(images);
            }
        }
        _ => {
            if tags.extras.images.is_none() {
                tags.extras.images = Some(images);
            }
        }
    }
}

fn set_license(stack: &mut [Frame], tags: &mut FeedTags, license: License) {
    match stack.last_mut() {
        Some(Frame::Item(it)) => {
            if it.license.is_none() {
                it.license = Some(license);
            }
        }
        Some(Frame::Live(live)) => {
            if live.tags.license.is_none() {
                live.tags.license = Some(license);
            }
        }
        _ => {
            if tags.extras.license.is_none() {
                tags.extras.license = Some(license);
            }
        }
    }
}

fn set_chat(stack: &mut [Frame], tags: &mut FeedTags, chat: Chat) {
    match stack.last_mut() {
        Some(Frame::Item(it)) => {
            if it.chat.is_none() {
                it.chat = Some(chat);
            }
        }
        Some(Frame::Live(live)) => {
            if live.tags.chat.is_none() {
                live.tags.chat = Some(chat);
            }
        }
        _ => {
            if tags.extras.chat.is_none() {
                tags.extras.chat = Some(chat);
            }
        }
    }
}

fn commit_text(
    kind: CapKind,
    e: &BytesStart,
    text: &str,
    stack: &mut Vec<Frame>,
    tags: &mut FeedTags,
) {
    let text = text.to_string();
    match kind {
        CapKind::Person => {
            let name = clean_name(&text);
            if name.is_empty() {
                return;
            }
            push_person(
                stack,
                tags,
                FeedPerson {
                    name,
                    role: attr(e, "role"),
                    group: attr(e, "group"),
                    img: http_url(attr(e, "img")),
                    href: http_url(attr(e, "href")),
                },
            );
        }
        CapKind::Funding => {
            if let Some(url) = http_url(attr(e, "url")) {
                push_funding(
                    stack,
                    tags,
                    Funding {
                        url,
                        description: text,
                    },
                );
            }
        }
        CapKind::Location => {
            if text.is_empty() {
                return;
            }
            push_location(
                stack,
                tags,
                Location {
                    name: text,
                    rel: attr(e, "rel"),
                    geo: attr(e, "geo"),
                    osm: attr(e, "osm"),
                    country: attr(e, "country"),
                },
            );
        }
        CapKind::Locked => {
            if !text.is_empty() && tags.extras.locked.is_none() {
                tags.extras.locked = Some(Locked {
                    value: text,
                    owner: attr(e, "owner"),
                });
            }
        }
        CapKind::Trailer => {
            if let Some(url) = http_url(attr(e, "url")) {
                tags.extras.trailers.push(Trailer {
                    title: text,
                    url,
                    pubdate: attr(e, "pubdate").unwrap_or_default(),
                    length: attr(e, "length"),
                    mime_type: attr(e, "type"),
                    season: attr(e, "season"),
                });
            }
        }
        CapKind::License => {
            let license = License {
                identifier: text.clone(),
                url: http_url(attr(e, "url")),
            };
            if license.identifier.is_empty() && license.url.is_none() {
                return;
            }
            set_license(stack, tags, license);
        }
        CapKind::Season => {
            if text.is_empty() {
                return;
            }
            if let Some(it) = item_tags_mut(stack) {
                if it.season.is_none() {
                    it.season = Some(Season {
                        number: text,
                        name: attr(e, "name"),
                    });
                }
            }
        }
        CapKind::Episode => {
            if text.is_empty() {
                return;
            }
            if let Some(it) = item_tags_mut(stack) {
                if it.episode.is_none() {
                    it.episode = Some(EpisodeNumber {
                        number: text,
                        display: attr(e, "display"),
                    });
                }
            }
        }
        CapKind::ContentLink => {
            if let Some(href) = safe_uri(attr(e, "href")) {
                if let Some(it) = item_tags_mut(stack) {
                    it.content_links.push(ContentLink { href, text });
                }
            }
        }
        CapKind::Txt => {
            let purpose = attr(e, "purpose");
            if text.is_empty() && purpose.is_none() {
                return;
            }
            push_txt(
                stack,
                tags,
                Txt {
                    purpose,
                    value: text,
                },
            );
        }
        CapKind::UpdateFrequency => {
            if tags.extras.update_frequency.is_none() {
                tags.extras.update_frequency = Some(UpdateFrequency {
                    text,
                    complete: strict_bool(e, "complete"),
                    dtstart: attr(e, "dtstart"),
                    rrule: attr(e, "rrule"),
                });
            }
        }
        CapKind::Complete => {
            if tags.extras.complete.is_none() {
                tags.extras.complete = Some(true);
            }
        }
        CapKind::Medium => {
            if !text.is_empty() && tags.extras.medium.is_none() {
                tags.extras.medium = Some(text);
            }
        }
        CapKind::Soundbite => {
            if let Some(it) = item_tags_mut(stack) {
                it.soundbites.push(Soundbite {
                    start_time: attr(e, "startTime").unwrap_or_default(),
                    duration: attr(e, "duration").unwrap_or_default(),
                    title: (!text.is_empty()).then_some(text),
                });
            }
        }
        CapKind::Block => {
            if text.is_empty() {
                return;
            }
            push_block(
                stack,
                tags,
                Block {
                    id: attr(e, "id"),
                    value: text,
                },
            );
        }
        CapKind::PodcastGuid => {
            if !text.is_empty() && tags.podcast_guid.is_none() {
                tags.podcast_guid = Some(text.clone());
                tags.extras.guid = Some(text);
            }
        }
        CapKind::ItemGuid => {
            if text.is_empty() {
                return;
            }
            if let Some(it) = item_tags_mut(stack) {
                if it.guid.is_none() {
                    it.guid = Some(text);
                }
            }
        }
        CapKind::Title => {
            if let Some(Frame::Live(live)) = stack.last_mut() {
                if live.title.is_none() && !text.is_empty() {
                    live.title = Some(text);
                }
            }
        }
        CapKind::Description => {
            if let Some(Frame::Live(live)) = stack.last_mut() {
                if live.description.is_none() && !text.is_empty() {
                    live.description = Some(text);
                }
            }
        }
        CapKind::Link => {
            if let Some(link) = safe_uri(Some(text)) {
                if let Some(Frame::Live(live)) = stack.last_mut() {
                    if live.link.is_none() {
                        live.link = Some(link);
                    }
                }
            }
        }
        CapKind::Author => {
            if let Some(Frame::Live(live)) = stack.last_mut() {
                if live.author.is_none() && !text.is_empty() {
                    live.author = Some(text);
                }
            }
        }
        CapKind::PubDate => {
            if let Some(Frame::Live(live)) = stack.last_mut() {
                if live.pub_date.is_none() && !text.is_empty() {
                    live.pub_date = Some(text);
                }
            }
        }
    }
}

fn handle_leaf(e: &BytesStart, podcast: bool, stack: &mut Vec<Frame>, tags: &mut FeedTags) {
    let local = local_name(e);
    if !podcast {
        if local == "enclosure" {
            let url = attr(e, "url");
            let length = attr(e, "length");
            let mime = attr(e, "type");
            if let Some(Frame::Live(live)) = stack.last_mut() {
                if live.tags.enclosure_url.is_none() {
                    live.tags.enclosure_url = url;
                }
                if live.enclosure_length.is_none() {
                    live.enclosure_length = length;
                }
                if live.enclosure_type.is_none() {
                    live.enclosure_type = mime;
                }
            } else if let Some(Frame::Item(it)) = stack.last_mut() {
                if it.enclosure_url.is_none() {
                    it.enclosure_url = url;
                }
            }
        }
        return;
    }
    match local {
        "chapters" => {
            if let Some(it) = item_tags_mut(stack) {
                if it.chapters_url.is_none() {
                    if let Some(url) = http_url(attr(e, "url")) {
                        it.chapters_url = Some(url);
                        it.chapters_type = attr(e, "type");
                    }
                }
            }
        }
        "transcript" => {
            if let Some(url) = http_url(attr(e, "url")) {
                if let Some(it) = item_tags_mut(stack) {
                    it.transcripts.push(Transcript {
                        url,
                        mime_type: attr(e, "type"),
                        language: attr(e, "language"),
                        rel: attr(e, "rel"),
                    });
                }
            }
        }
        "image" => {
            if let Some(href) = http_url(attr(e, "href")) {
                push_image(
                    stack,
                    tags,
                    PodcastImage {
                        href,
                        alt: attr(e, "alt"),
                        aspect_ratio: attr(e, "aspect-ratio"),
                        width: attr(e, "width"),
                        height: attr(e, "height"),
                        mime_type: attr(e, "type"),
                        purpose: attr(e, "purpose"),
                    },
                );
            }
        }
        "images" => {
            if let Some(raw) = attr(e, "srcset") {
                let set = parse_srcset(&raw);
                if !set.sources.is_empty() {
                    set_images(stack, tags, set);
                }
            }
        }
        "socialInteract" => {
            let protocol = attr(e, "protocol").unwrap_or_default();
            let uri = safe_uri(attr(e, "uri"));
            if protocol.is_empty() && uri.is_none() {
                return;
            }
            push_social(
                stack,
                tags,
                SocialInteract {
                    protocol,
                    uri,
                    account_id: attr(e, "accountId"),
                    account_url: http_url(attr(e, "accountUrl")),
                    priority: attr(e, "priority"),
                },
            );
        }
        "chat" => {
            let Some(server) = attr(e, "server") else {
                return;
            };
            let Some(protocol) = attr(e, "protocol") else {
                return;
            };
            set_chat(
                stack,
                tags,
                Chat {
                    server,
                    protocol,
                    account_id: attr(e, "accountId"),
                    space: attr(e, "space"),
                    embed_url: http_url(attr(e, "embedUrl")),
                },
            );
        }
        "podping" => {
            if tags.extras.podping.is_none() {
                tags.extras.podping = Some(Podping {
                    uses_podping: strict_bool(e, "usesPodping"),
                });
            }
        }
        "remoteItem" => {
            if let Some(item) = remote_from(e) {
                push_remote(stack, tags, item);
            }
        }
        "valueRecipient" => {
            let recipient = ValueRecipient {
                name: attr(e, "name").unwrap_or_default(),
                kind: attr(e, "type").unwrap_or_default(),
                address: attr(e, "address").unwrap_or_default(),
                split: attr(e, "split").unwrap_or_default(),
                custom_key: attr(e, "customKey"),
                custom_value: attr(e, "customValue"),
                fee: strict_bool(e, "fee"),
            };
            match stack.last_mut() {
                Some(Frame::Split(split)) => split.recipients.push(recipient),
                Some(Frame::Value(block)) => block.recipients.push(recipient),
                _ => {}
            }
        }
        "source" => {
            if let Some(Frame::Alt(alt)) = stack.last_mut() {
                if let Some(uri) = safe_uri(attr(e, "uri")) {
                    alt.sources.push(Source {
                        uri,
                        content_type: attr(e, "contentType"),
                    });
                }
            }
        }
        "integrity" => {
            if let Some(Frame::Alt(alt)) = stack.last_mut() {
                if alt.integrity.is_none() {
                    alt.integrity = Some(Integrity {
                        kind: attr(e, "type").unwrap_or_default(),
                        value: attr(e, "value").unwrap_or_default(),
                    });
                }
            }
        }
        _ => {}
    }
}

fn remote_from(e: &BytesStart) -> Option<RemoteItem> {
    Some(RemoteItem {
        feed_guid: attr(e, "feedGuid")?,
        feed_url: http_url(attr(e, "feedUrl")),
        item_guid: attr(e, "itemGuid"),
        medium: attr(e, "medium"),
        title: attr(e, "title"),
    })
}

fn push_remote(stack: &mut [Frame], tags: &mut FeedTags, item: RemoteItem) {
    match stack.last_mut() {
        Some(Frame::Split(split)) => {
            if split.remote_item.is_none() {
                split.remote_item = Some(item);
            }
        }
        Some(Frame::Podroll) => tags.extras.podroll.push(item),
        Some(Frame::Publisher) => {
            if tags.extras.publisher.is_none() {
                tags.extras.publisher = Some(item);
            }
        }
        Some(Frame::Item(it)) => it.remote_items.push(item),
        Some(Frame::Live(live)) => live.tags.remote_items.push(item),
        Some(Frame::Value(_)) | Some(Frame::Alt(_)) | Some(Frame::LiveValue(_)) => {}
        None => tags.extras.remote_items.push(item),
    }
}

/// Name key for matching people: accent-folded, lowercased, punctuation dropped, whitespace
/// collapsed. Must stay identical to `normalize_name` in database_functions/migration_definitions.py
/// (which backfills existing rows) and to PodPeopleDB's.
pub fn normalize_name(name: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    let folded: String = name
        .nfkd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .flat_map(|c| c.to_lowercase())
        .map(|c| match c {
            'ß' => 's',
            'ø' => 'o',
            'æ' => 'a',
            'đ' | 'ð' => 'd',
            'ł' => 'l',
            other => other,
        })
        .collect();
    let mut out = String::with_capacity(folded.len());
    for c in folded.chars() {
        if c.is_alphanumeric() {
            out.push(c);
        } else if c.is_whitespace() || c == '-' || c == '_' {
            out.push(' ');
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Lowercased Podcasting 2.0 role; missing means "host" per the spec.
pub fn normalize_role(role: Option<&str>) -> String {
    let r = role.unwrap_or("").trim().to_lowercase();
    let r = r.split_whitespace().collect::<Vec<_>>().join(" ");
    match r.as_str() {
        "" | "hosts" => "host".into(),
        "cohost" | "co host" => "co-host".into(),
        "guests" => "guest".into(),
        _ => r.chars().take(60).collect(),
    }
}

/// `pending`, `live`, or `ended`. Any other status is treated as upcoming.
pub fn normalize_live_status(status: &str) -> &'static str {
    match status.trim().to_ascii_lowercase().as_str() {
        "live" => "live",
        "ended" => "ended",
        _ => "pending",
    }
}

/// HLS when the enclosure type says MPEG-URL, or the URL path ends in `.m3u8` / `.m3u`.
pub fn live_stream_is_hls(mime: Option<&str>, url: &str) -> bool {
    let mime = mime.unwrap_or("").trim().to_ascii_lowercase();
    if mime.contains("mpegurl") || mime.contains("m3u8") {
        return true;
    }
    let path = url.split(['?', '#']).next().unwrap_or(url).to_ascii_lowercase();
    path.ends_with(".m3u8") || path.ends_with(".m3u")
}

/// Video element when the enclosure type is `video/*` or DASH. HLS stays audio unless the type says video.
pub fn live_stream_is_video(mime: Option<&str>) -> bool {
    let mime = mime.unwrap_or("").trim().to_ascii_lowercase();
    mime.starts_with("video/") || mime.contains("dash")
}

/// Hide a live item once it is over.
///
/// `status="ended"` is enough. An end time in the past hides the item too, even
/// when the status is still `live` or `pending` and even when no stored episode
/// guid matches. `episode_guids` is accepted so existing callers keep compiling.
/// Order is live, then upcoming, by start time.
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
        .and_then(parse_dtstart)
        .is_some_and(|end| end < Utc::now())
}

/// Episode response for `/api/data/fetch_podcasting_2_data`.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct EpisodeDocument {
    /// Chapter objects fetched from `tags.chapters_url` (`chapters` array of the JSON file).
    pub chapters: serde_json::Value,
    pub transcripts: Vec<Transcript>,
    /// Resolved credits: the episode's people, plus the show's regulars when the episode
    /// doesn't name its own hosts.
    pub people: Vec<crate::services::people::CreditedPerson>,
    /// Tags that were on this `<item>`. Channel-level people are not copied here.
    pub tags: ItemTags,
}

/// Podcast response for `/api/data/fetch_podcasting_2_pod_data`.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ChannelDocument {
    pub people: Vec<crate::services::people::CreditedPerson>,
    pub guests: Vec<crate::services::people::CreditedPerson>,
    pub podroll: Vec<RemoteItem>,
    pub funding: Vec<Funding>,
    pub value: Vec<ValueBlock>,
    pub guid: Option<String>,
    /// Full channel document. `podroll`, `funding` and `value` are the same objects as the
    /// top-level keys.
    pub tags: ChannelExtras,
}

#[cfg(test)]
mod tests {
    use super::*;

    const FEED: &str = r#"<rss xmlns:podcast="https://podcastindex.org/namespace/1.0"><channel>
<title>T</title>
<podcast:guid>abc-123</podcast:guid>
<podcast:person role="host" img="https://x/a.jpg">Alice &amp; Co</podcast:person>
<podcast:person
   role="Guest">Bob
   Builder</podcast:person>
<podcast:person href="javascript:alert(1)">Mallory</podcast:person>
<podcast:funding url="https://fund.example">Support us</podcast:funding>
<podcast:podroll><podcast:remoteItem feedGuid="g-1"/><podcast:remoteItem feedGuid="g-2"/></podcast:podroll>
<podcast:value type="lightning" method="keysend" suggested="0.00000005000">
  <podcast:valueRecipient name="Alice" type="node" address="03ab" split="90"/>
  <podcast:valueRecipient name="App" type="node" address="02cd" split="10"/>
</podcast:value>
<item><title>E2</title><guid isPermaLink="false">e-2</guid><enclosure url="https://cdn/e2.mp3?a=1&amp;b=2"/>
  <podcast:person role="guest">Carol</podcast:person>
  <podcast:chapters url="https://cdn/e2.json" type="application/json+chapters"/>
  <podcast:transcript url="https://cdn/e2.srt" type="application/srt" language="en"/>
</item>
<item><title>E1</title><guid>e-1</guid><enclosure url="https://cdn/e1.mp3"/></item>
</channel></rss>"#;

    #[test]
    fn parses_everything() {
        let t = parse(FEED);
        assert_eq!(t.podcast_guid.as_deref(), Some("abc-123"));
        assert_eq!(t.extras.guid.as_deref(), Some("abc-123"));
        assert_eq!(t.people.len(), 3);
        assert_eq!(t.people[0].name, "Alice & Co");
        assert_eq!(t.people[0].img.as_deref(), Some("https://x/a.jpg"));
        assert_eq!(t.people[1].name, "Bob Builder");
        assert_eq!(t.people[2].href, None, "non-http links are dropped");
        assert_eq!(
            t.extras.funding,
            vec![Funding {
                url: "https://fund.example".into(),
                description: "Support us".into()
            }]
        );
        assert_eq!(t.extras.podroll.len(), 2);
        assert_eq!(t.extras.value[0].recipients.len(), 2);
        assert_eq!(t.items.len(), 1, "items without tags aren't kept");
        let it = &t.items[0];
        assert_eq!(it.guid.as_deref(), Some("e-2"));
        assert_eq!(
            it.enclosure_url.as_deref(),
            Some("https://cdn/e2.mp3?a=1&b=2")
        );
        assert_eq!(it.people[0].name, "Carol");
        assert_eq!(it.chapters_url.as_deref(), Some("https://cdn/e2.json"));
        assert_eq!(
            it.chapters_type.as_deref(),
            Some("application/json+chapters")
        );
        assert_eq!(it.transcripts[0].language.as_deref(), Some("en"));
        assert!(t.has_people());
        assert_eq!(
            parse(&FEED.replace('\n', "")).fingerprint(),
            t.fingerprint(),
            "layout doesn't matter"
        );
    }

    #[test]
    fn normalizes() {
        assert_eq!(normalize_name("  Griffin   McElroy "), "griffin mcelroy");
        assert_eq!(normalize_name("Beyoncé"), "beyonce");
        assert_eq!(normalize_name("J.R.R. Tolkien"), "jrr tolkien");
        assert_eq!(normalize_role(Some("Host")), "host");
        assert_eq!(normalize_role(None), "host");
        assert_eq!(normalize_role(Some("CoHost")), "co-host");
    }

    #[test]
    fn old_channel_json_still_loads() {
        let old = r#"{"podroll":[{"feed_guid":"g"}],"funding":[{"url":"https://f.example","description":"d"}],"value":[{"type":"lightning","method":"keysend","suggested":null,"recipients":[{"name":"A","type":"node","address":"03","split":"100"}]}]}"#;
        let extras: ChannelExtras = serde_json::from_str(old).unwrap();
        assert_eq!(extras.podroll[0].feed_guid, "g");
        assert_eq!(extras.funding[0].description, "d");
        assert_eq!(extras.value[0].kind, "lightning");
        assert!(extras.value[0].time_splits.is_empty());
        assert!(extras.locked.is_none());
        assert!(extras.medium.is_none());
        let back = serde_json::to_value(&extras).unwrap();
        assert_eq!(back["value"][0]["recipients"][0]["split"], "100");
        assert!(back.get("locked").is_none());
    }

    #[test]
    fn namespace_is_required_and_either_uri_works() {
        let feed = r#"<rss xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd" xmlns:pc="https://podcastindex.org/namespace/1.0">
          <channel>
            <itunes:block>yes</itunes:block>
            <person>Nope</person>
            <pc:block>no</pc:block>
            <pc:medium>music</pc:medium>
            <pc:guid>11111111-1111-5111-8111-111111111111</pc:guid>
            <item>
              <guid>g</guid>
              <enclosure url="https://cdn/a.mp3"/>
              <itunes:episode>4</itunes:episode>
              <itunes:season>9</itunes:season>
              <pc:episode display="Shown">8</pc:episode>
            </item>
          </channel></rss>"#;
        let t = parse(feed);
        assert_eq!(t.extras.blocks.len(), 1);
        assert_eq!(t.extras.blocks[0].value, "no");
        assert_eq!(t.extras.medium.as_deref(), Some("music"));
        assert_eq!(
            t.podcast_guid.as_deref(),
            Some("11111111-1111-5111-8111-111111111111")
        );
        assert!(t.people.is_empty());
        assert_eq!(t.items[0].episode.as_ref().unwrap().number, "8");
        assert_eq!(
            t.items[0].episode.as_ref().unwrap().display.as_deref(),
            Some("Shown")
        );
        assert!(t.items[0].season.is_none());

        let alt = r#"<rss xmlns:podcast="https://github.com/Podcastindex-org/podcast-namespace/blob/main/docs/1.0.md">
          <channel><podcast:medium>podcast</podcast:medium></channel></rss>"#;
        assert_eq!(parse(alt).extras.medium.as_deref(), Some("podcast"));
    }

    #[test]
    fn value_time_split_without_root_recipients_is_kept() {
        let feed = r#"<rss xmlns:podcast="https://podcastindex.org/namespace/1.0"><channel>
          <item><guid>e</guid><enclosure url="https://cdn/a.mp3"/>
            <podcast:value type="lightning" method="keysend">
              <podcast:valueTimeSplit startTime="1.5" duration="2">
                <podcast:remoteItem feedGuid="feed-g" itemGuid="item-g" medium="music"/>
              </podcast:valueTimeSplit>
            </podcast:value>
          </item></channel></rss>"#;
        let t = parse(feed);
        let value = &t.items[0].value[0];
        assert!(value.recipients.is_empty());
        assert_eq!(value.time_splits[0].start_time, "1.5");
        assert_eq!(
            value.time_splits[0].remote_item.as_ref().unwrap().feed_guid,
            "feed-g"
        );
        assert_eq!(
            value.time_splits[0]
                .remote_item
                .as_ref()
                .unwrap()
                .item_guid
                .as_deref(),
            Some("item-g")
        );
    }

    #[test]
    fn fixture_feed_covers_every_tag() {
        let t = parse(include_str!(
            "../../tests/fixtures/podcasting2-namespace.xml"
        ));
        assert_eq!(
            t.podcast_guid.as_deref(),
            Some("917393e3-1b1e-5cef-ace4-edaa54e1f810")
        );
        assert_eq!(t.people[0].name, "Ada & Host");
        let x = &t.extras;
        assert_eq!(x.locked.as_ref().unwrap().value, "yes");
        assert_eq!(
            x.locked.as_ref().unwrap().owner.as_deref(),
            Some("owner@example.invalid")
        );
        assert_eq!(x.funding[0].description, "Support the show");
        assert_eq!(x.locations.len(), 2);
        assert_eq!(x.locations[0].geo.as_deref(), Some("geo:30.2672,-97.7431"));
        assert_eq!(x.locations[0].osm.as_deref(), Some("R113314"));
        assert_eq!(x.locations[0].country.as_deref(), Some("US"));
        assert_eq!(x.trailers.len(), 2);
        assert_eq!(x.trailers[1].season.as_deref(), Some("4"));
        assert_eq!(x.trailers[0].mime_type.as_deref(), Some("audio/mpeg"));
        assert_eq!(
            x.license.as_ref().unwrap().identifier,
            "my-podcast-license-v1"
        );
        assert_eq!(x.medium.as_deref(), Some("music"));
        assert_eq!(x.images.as_ref().unwrap().sources.len(), 2);
        assert_eq!(
            x.images.as_ref().unwrap().sources[0].descriptor.as_deref(),
            Some("1500w")
        );
        assert_eq!(x.image.len(), 2);
        assert_eq!(x.image[0].aspect_ratio.as_deref(), Some("1/1"));
        assert_eq!(x.image[0].purpose.as_deref(), Some("artwork"));
        assert_eq!(x.image[1].purpose.as_deref(), Some("artwork social"));
        assert_eq!(x.social_interacts.len(), 2);
        assert_eq!(x.social_interacts[1].protocol, "disabled");
        assert!(x.social_interacts[1].uri.is_none());
        assert_eq!(x.blocks.len(), 2);
        assert_eq!(x.blocks[1].id.as_deref(), Some("google"));
        assert_eq!(x.txts[0].purpose.as_deref(), Some("verify"));
        assert_eq!(
            x.update_frequency.as_ref().unwrap().rrule.as_deref(),
            Some("FREQ=WEEKLY;INTERVAL=2")
        );
        assert_eq!(x.update_frequency.as_ref().unwrap().text, "Biweekly");
        assert_eq!(x.podping.as_ref().unwrap().uses_podping, Some(true));
        assert_eq!(x.chat.as_ref().unwrap().protocol, "irc");
        assert_eq!(x.chat.as_ref().unwrap().space.as_deref(), Some("#show"));
        assert_eq!(
            x.chat.as_ref().unwrap().embed_url.as_deref(),
            Some("https://kiwiirc.example.invalid/nextclient/")
        );
        assert_eq!(x.complete, Some(true));
        assert_eq!(x.podroll[0].title.as_deref(), Some("Buzzcast"));
        assert_eq!(
            x.podroll[0].feed_url.as_deref(),
            Some("https://example.invalid/rec.xml")
        );
        assert_eq!(x.remote_items[0].item_guid.as_deref(), Some("song-1"));
        assert_eq!(x.remote_items[0].medium.as_deref(), Some("music"));
        assert_eq!(
            x.publisher.as_ref().unwrap().medium.as_deref(),
            Some("publisher")
        );
        assert_eq!(
            x.value[0].recipients[0].custom_key.as_deref(),
            Some("696969")
        );
        assert_eq!(x.value[0].recipients[1].fee, Some(true));
        assert_eq!(
            x.value[0].time_splits[0].remote_percentage.as_deref(),
            Some("95")
        );
        assert_eq!(
            x.value[0].time_splits[0]
                .remote_item
                .as_ref()
                .unwrap()
                .title
                .as_deref(),
            Some("Split Song")
        );

        let live = &x.live_items[0];
        assert_eq!(live.status, "live");
        assert_eq!(live.start, "2026-10-21T18:00:00.000Z");
        assert_eq!(live.end.as_deref(), Some("2026-10-21T19:00:00.000Z"));
        assert_eq!(live.title.as_deref(), Some("Live Now"));
        assert_eq!(live.link.as_deref(), Some("https://example.invalid/live"));
        assert_eq!(live.tags.guid.as_deref(), Some("live-1"));
        assert_eq!(live.author.as_deref(), Some("Ada"));
        assert_eq!(live.enclosure_type.as_deref(), Some("audio/mpeg"));
        assert_eq!(live.enclosure_length.as_deref(), Some("312"));
        assert_eq!(
            live.tags.enclosure_url.as_deref(),
            Some("https://example.invalid/live.mp3")
        );
        assert_eq!(live.tags.people[0].name, "Live Host");
        assert_eq!(live.tags.alternate_enclosures[0].default, Some(true));
        assert_eq!(live.tags.content_links[0].text, "Listen live");
        assert_eq!(
            live.tags.funding[0].url,
            "https://example.invalid/live-fund"
        );
        assert_eq!(live.tags.value[0].recipients[0].kind, "lnaddress");
        assert!(live.tags.chat.is_some());
        let live_value = live.live_value.as_ref().unwrap();
        assert_eq!(live_value.protocol, "socket.io");
        assert_eq!(
            live_value.uri,
            "https://live.example.invalid/socket.io/"
        );
        assert_eq!(live_value.value[0].recipients[0].name, "Live Split");

        assert_eq!(t.items.len(), 1, "the plain episode has only itunes tags");
        let it = &t.items[0];
        assert_eq!(it.guid.as_deref(), Some("ep-tagged"));
        assert_eq!(
            it.enclosure_url.as_deref(),
            Some("https://cdn.example.invalid/audio/tagged.mp3?x=1&y=2")
        );
        assert_eq!(it.people[0].name, "Gus Guest");
        assert_eq!(it.transcripts.len(), 2);
        assert_eq!(it.transcripts[0].rel.as_deref(), Some("captions"));
        assert_eq!(
            it.chapters_type.as_deref(),
            Some("application/json+chapters")
        );
        assert_eq!(it.soundbites.len(), 2);
        assert_eq!(it.soundbites[1].title.as_deref(), Some("Rock & Roll"));
        assert_eq!(it.locations[0].name, "New York");
        assert_eq!(it.season.as_ref().unwrap().number, "4");
        assert_eq!(
            it.season.as_ref().unwrap().name.as_deref(),
            Some("The Pilot")
        );
        assert_eq!(it.episode.as_ref().unwrap().number, "12");
        assert_eq!(
            it.episode.as_ref().unwrap().display.as_deref(),
            Some("Ch.3")
        );
        assert_eq!(it.license.as_ref().unwrap().identifier, "CC-BY-NC-ND-4.0");
        assert_eq!(it.alternate_enclosures.len(), 2);
        assert_eq!(
            it.alternate_enclosures[0].codecs.as_deref(),
            Some("mp4a.40.2")
        );
        assert_eq!(it.alternate_enclosures[0].sources.len(), 2);
        assert_eq!(
            it.alternate_enclosures[0].sources[1].uri,
            "ipfs://bafybeigdyr"
        );
        assert_eq!(
            it.alternate_enclosures[0].sources[1]
                .content_type
                .as_deref(),
            Some("audio/mpeg")
        );
        assert_eq!(
            it.alternate_enclosures[0].integrity.as_ref().unwrap().kind,
            "sri"
        );
        assert_eq!(it.image[0].width.as_deref(), Some("800"));
        assert_eq!(
            it.images.as_ref().unwrap().sources[1].descriptor.as_deref(),
            Some("200w")
        );
        assert_eq!(it.content_links.len(), 2);
        assert_eq!(it.social_interacts[0].priority.as_deref(), Some("2"));
        assert_eq!(it.blocks[0].id.as_deref(), Some("amazon"));
        assert_eq!(it.txts[0].value, "false");
        assert_eq!(it.funding[0].description, "Tip this episode");
        assert_eq!(it.chat.as_ref().unwrap().protocol, "matrix");
        assert_eq!(
            it.remote_items[0].feed_guid,
            "b40ffcf7-2c48-5cfe-8daa-b65d766b2c25"
        );
        assert_eq!(it.value[0].suggested.as_deref(), Some("0.00000001000"));
        assert_eq!(
            it.value[0].time_splits[0].remote_start_time.as_deref(),
            Some("10")
        );
        assert_eq!(it.value[0].time_splits[0].recipients[0].name, "Guest Split");
        assert_eq!(
            it.value[0].time_splits[0]
                .remote_item
                .as_ref()
                .map(|r| r.feed_guid.as_str()),
            None
        );
    }

    #[test]
    fn refresh_plan_respects_rrule_complete_and_dtstart() {
        let now = DateTime::parse_from_rfc3339("2026-06-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert!(refresh_plan(&None, now).interval_minutes.is_none());

        let weekly = UpdateFrequency {
            text: "Weekly".into(),
            complete: None,
            dtstart: None,
            rrule: Some("FREQ=WEEKLY".into()),
        };
        let plan = refresh_plan(&Some(weekly), now);
        assert_eq!(plan.interval_minutes, Some(7 * 24 * 60));

        let bi = UpdateFrequency {
            text: String::new(),
            complete: None,
            dtstart: None,
            rrule: Some("FREQ=WEEKLY;INTERVAL=2".into()),
        };
        assert_eq!(
            refresh_plan(&Some(bi), now).interval_minutes,
            Some(14 * 24 * 60)
        );

        let hourly = UpdateFrequency {
            text: String::new(),
            complete: None,
            dtstart: None,
            rrule: Some("FREQ=HOURLY".into()),
        };
        assert_eq!(refresh_plan(&Some(hourly), now).interval_minutes, Some(60));

        let yearly = UpdateFrequency {
            text: String::new(),
            complete: None,
            dtstart: None,
            rrule: Some("FREQ=YEARLY".into()),
        };
        assert_eq!(
            refresh_plan(&Some(yearly), now).interval_minutes,
            Some(30 * 24 * 60)
        );

        let done = UpdateFrequency {
            text: "done".into(),
            complete: Some(true),
            dtstart: None,
            rrule: Some("FREQ=DAILY".into()),
        };
        assert_eq!(
            refresh_plan(&Some(done), now).interval_minutes,
            Some(7 * 24 * 60)
        );

        let future = UpdateFrequency {
            text: String::new(),
            complete: None,
            dtstart: Some("2026-06-03T00:00:00Z".into()),
            rrule: Some("FREQ=WEEKLY".into()),
        };
        let plan = refresh_plan(&Some(future), now);
        assert_eq!(plan.interval_minutes, Some(7 * 24 * 60));
        assert_eq!(
            plan.next_at.unwrap(),
            DateTime::parse_from_rfc3339("2026-06-03T00:00:00Z").unwrap()
        );

        let far = UpdateFrequency {
            text: String::new(),
            complete: None,
            dtstart: Some("2028-01-01T00:00:00Z".into()),
            rrule: Some("FREQ=DAILY".into()),
        };
        let plan = refresh_plan(&Some(far), now);
        assert_eq!(plan.next_at.unwrap(), now + Duration::days(30));

        let prose = UpdateFrequency {
            text: "whenever".into(),
            complete: None,
            dtstart: None,
            rrule: None,
        };
        assert!(refresh_plan(&Some(prose), now).next_at.is_none());
    }

    fn sample_live(status: &str, guid: Option<&str>, start: &str) -> LiveItem {
        LiveItem {
            status: status.into(),
            start: start.into(),
            tags: ItemTags {
                guid: guid.map(str::to_string),
                ..ItemTags::default()
            },
            ..LiveItem::default()
        }
    }

    #[test]
    fn ended_live_item_hides_when_the_episode_exists() {
        let items = vec![
            sample_live("ended", Some("Live-1"), "2026-01-03T00:00:00Z"),
            sample_live("pending", Some("soon"), "2026-02-02T00:00:00Z"),
            sample_live("LIVE", Some("Live-1"), "2026-02-01T00:00:00Z"),
            sample_live("ended", None, "2026-01-01T00:00:00Z"),
            sample_live("ended", Some("kept"), "2026-01-02T00:00:00Z"),
            sample_live("soon", Some("later"), "2026-03-01T00:00:00Z"),
        ];
        let visible = visible_live_items(&items, &[" live-1 ".into()]);
        let labels: Vec<_> = visible
            .iter()
            .map(|item| {
                (
                    normalize_live_status(&item.status),
                    item.tags.guid.clone(),
                )
            })
            .collect();
        assert_eq!(
            labels,
            vec![
                ("live", Some("Live-1".into())),
                ("pending", Some("soon".into())),
                ("pending", Some("later".into())),
            ]
        );
        let mut past = sample_live("live", Some("old"), "2020-01-01T00:00:00Z");
        past.end = Some("2020-01-01T01:00:00Z".into());
        let mut future = sample_live("pending", Some("next"), "2099-01-01T00:00:00Z");
        future.end = Some("2099-01-01T02:00:00Z".into());
        let ended = sample_live("ended", Some("no-match"), "2024-01-01T00:00:00Z");
        let visible = visible_live_items(&[past, future, ended], &[]);
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].tags.guid.as_deref(), Some("next"));
        assert!(live_stream_is_hls(
            Some("application/x-mpegURL"),
            "https://cdn.example/live"
        ));
        assert!(live_stream_is_hls(
            Some("application/vnd.apple.mpegurl"),
            "https://cdn.example/live"
        ));
        assert!(live_stream_is_hls(
            Some("audio/mpeg"),
            "https://cdn.example/live.m3u8?token=1"
        ));
        assert!(!live_stream_is_hls(
            Some("audio/mpeg"),
            "https://cdn.example/live.mp3"
        ));
        assert!(live_stream_is_video(Some("video/mp4")));
        assert!(live_stream_is_video(Some("application/dash+xml")));
        assert!(!live_stream_is_video(Some("application/x-mpegURL")));
    }
}
