//! Podcasting 2.0 podroll and value blocks on the podcast page.
//!
//! The server already returns this from `/api/data/fetch_podcasting_2_pod_data`. Podroll items
//! are usually just a feed GUID (and a feed URL when the server stored one). Title and artwork
//! come from the same Podcast Index search the rest of the app uses (`/api/data/proxy_search`).
//! Value is display-only: recipients and split percentages, no wallet and no payments.

use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

use i18nrs::yew::use_translation;
use wasm_bindgen_futures::spawn_local;
use web_sys::MouseEvent;
use yew::prelude::*;
use yew_router::history::BrowserHistory;
use yewdux::prelude::*;

use crate::components::click_events::create_on_title_click;
use crate::components::context::{AppState, NotificationState};
use crate::components::gen_components::FallbackImage;
use crate::components::people::use_auth;
use crate::requests::pod_req::{
    call_add_podcast, call_check_podcast, PodrollItem, PodcastValues, Value, ValueRecipient,
};
use crate::requests::search_pods::{call_get_podcast_info, Podcast};

use web_sys::{HtmlAudioElement, HtmlVideoElement};

use crate::components::audio::{on_play_click, play_live_stream};
use crate::components::context::UIState;
use crate::requests::episode::Episode;
use crate::requests::podcasting2::{
    artwork_choice, cadence_of, chat_url, format_clock, format_numbers_badge, http_link, location_map_url,
    live_playback, normalize_live_status, resolve_numbers, safe_link, social_url, soundbite_window,
    time_splits_of, trailer_groups, trailer_is_video, visible_live_items, ArtworkChoice, Cadence,
    ChannelTags, EpisodeNumber, ImageSet, ItemTags, PodcastImage, RemoteItem, Season,
};

/// A Podcast Index (or search-proxy) hit reduced to the fields the podroll needs.
#[derive(Clone, Debug, PartialEq)]
pub struct IndexHit {
    pub guid: String,
    pub title: String,
    pub url: String,
    pub original_url: String,
    pub artwork: String,
    pub author: String,
    pub description: String,
    pub link: String,
    pub id: i32,
    pub episode_count: i32,
    pub explicit: bool,
    pub categories: HashMap<String, String>,
}

impl IndexHit {
    pub fn from_podcast(podcast: &Podcast) -> Self {
        let artwork = if podcast.artwork.trim().is_empty() {
            podcast.image.clone()
        } else {
            podcast.artwork.clone()
        };
        Self {
            guid: podcast.podcastGuid.clone().unwrap_or_default(),
            title: podcast.title.clone(),
            url: podcast.url.clone(),
            original_url: podcast.originalUrl.clone(),
            artwork,
            author: podcast.author.clone(),
            description: podcast.description.clone(),
            link: podcast.link.clone(),
            id: podcast.id,
            episode_count: podcast.episodeCount,
            explicit: podcast.explicit,
            categories: podcast.categories.clone().unwrap_or_default(),
        }
    }
}

#[derive(Clone, PartialEq)]
struct ResolvedShow {
    feed_guid: String,
    feed_url: String,
    title: String,
    author: String,
    artwork: String,
    description: String,
    link: String,
    index_id: i32,
    episode_count: i32,
    explicit: bool,
    categories: HashMap<String, String>,
    resolved: bool,
    subscribed: bool,
    subscribing: bool,
}

/// Search term for the existing Podcast Index proxy. A feed URL looks up reliably; a GUID is
/// only accepted later when a result's `podcastGuid` matches, so a loose text search cannot
/// attach the wrong show.
pub fn lookup_query(feed_guid: &str, feed_url: Option<&str>) -> String {
    match nonempty(feed_url) {
        Some(url) => url.to_string(),
        None => feed_guid.trim().to_string(),
    }
}

pub fn format_split(split: &str) -> String {
    let trimmed = split.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.ends_with('%') {
        return trimmed.to_string();
    }
    if let Ok(n) = trimmed.parse::<f64>() {
        if n.is_finite() && n.fract() == 0.0 {
            return format!("{}%", n as i64);
        }
        return format!("{trimmed}%");
    }
    trimmed.to_string()
}

pub fn short_guid(guid: &str) -> String {
    let guid = guid.trim();
    if guid.is_empty() {
        return String::new();
    }
    if guid.chars().count() <= 12 {
        return guid.to_string();
    }
    let prefix: String = guid.chars().take(8).collect();
    format!("{prefix}…")
}

pub fn podroll_title(resolved: Option<&str>, feed_url: Option<&str>, feed_guid: &str) -> String {
    if let Some(title) = nonempty(resolved) {
        return title.to_string();
    }
    if let Some(url) = nonempty(feed_url) {
        return url_host(url).unwrap_or_else(|| url.to_string());
    }
    short_guid(feed_guid)
}

/// Pick the search hit for one podroll item.
///
/// GUID equality wins. Otherwise an exact feed URL match wins. A single result is kept only
/// when we searched by feed URL — Podcast Index term search returns that feed for an exact URL,
/// sometimes with a normalized URL. A GUID search that does not echo the GUID is ignored.
pub fn pick_index_hit<'a>(
    hits: &'a [IndexHit],
    feed_guid: &str,
    feed_url: Option<&str>,
) -> Option<&'a IndexHit> {
    let guid = feed_guid.trim();
    if !guid.is_empty() {
        if let Some(hit) = hits.iter().find(|hit| hit.guid.eq_ignore_ascii_case(guid)) {
            return Some(hit);
        }
    }
    let Some(url) = nonempty(feed_url) else {
        return None;
    };
    if let Some(hit) = hits
        .iter()
        .find(|hit| urls_match(&hit.url, url) || urls_match(&hit.original_url, url))
    {
        return Some(hit);
    }
    if hits.len() == 1 {
        return hits.first();
    }
    None
}

fn nonempty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn urls_match(left: &str, right: &str) -> bool {
    normalize_url(left) == normalize_url(right) && !normalize_url(left).is_empty()
}

fn normalize_url(url: &str) -> String {
    url.trim().trim_end_matches('/').to_ascii_lowercase()
}

fn url_host(url: &str) -> Option<String> {
    url::Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(|host| host.to_string()))
}

fn placeholder(item: &PodrollItem) -> ResolvedShow {
    let feed_url = item.feed_url.clone().unwrap_or_default();
    let title = podroll_title(item.title.as_deref(), item.feed_url.as_deref(), &item.feed_guid);
    let artwork = item.artwork_url.clone().unwrap_or_default();
    ResolvedShow {
        feed_guid: item.feed_guid.clone(),
        feed_url,
        title,
        author: String::new(),
        artwork,
        description: String::new(),
        link: String::new(),
        index_id: 0,
        episode_count: 0,
        explicit: false,
        categories: HashMap::new(),
        resolved: nonempty(item.title.as_deref()).is_some(),
        subscribed: false,
        subscribing: false,
    }
}

fn apply_hit(mut show: ResolvedShow, hit: &IndexHit) -> ResolvedShow {
    if !hit.title.trim().is_empty() {
        show.title = hit.title.clone();
        show.resolved = true;
    }
    if !hit.url.trim().is_empty() {
        show.feed_url = hit.url.clone();
    } else if !hit.original_url.trim().is_empty() {
        show.feed_url = hit.original_url.clone();
    }
    if !hit.artwork.trim().is_empty() {
        show.artwork = hit.artwork.clone();
    }
    if !hit.author.trim().is_empty() {
        show.author = hit.author.clone();
    }
    if !hit.description.trim().is_empty() {
        show.description = hit.description.clone();
    }
    if !hit.link.trim().is_empty() {
        show.link = hit.link.clone();
    }
    if hit.id > 0 {
        show.index_id = hit.id;
    }
    show.episode_count = hit.episode_count;
    show.explicit = hit.explicit;
    if !hit.categories.is_empty() {
        show.categories = hit.categories.clone();
    }
    show
}

#[derive(Properties, PartialEq, Clone)]
pub struct Podcasting2ExtrasProps {
    pub podroll: Vec<PodrollItem>,
    pub value: Vec<Value>,
}

#[function_component(Podcasting2Extras)]
pub fn podcasting2_extras(props: &Podcasting2ExtrasProps) -> Html {
    let (i18n, _) = use_translation();
    let auth = use_auth();
    let user_id = *use_selector(|state: &AppState| {
        state
            .user_details
            .as_ref()
            .map(|details| details.UserID)
            .unwrap_or(0)
    });
    let shows = use_state(|| {
        props
            .podroll
            .iter()
            .filter(|item| !item.feed_guid.trim().is_empty() || nonempty(item.feed_url.as_deref()).is_some())
            .map(placeholder)
            .collect::<Vec<_>>()
    });

    {
        let shows = shows.clone();
        let podroll = props.podroll.clone();
        let auth = auth.clone();
        use_effect_with((podroll, auth, user_id), move |(podroll, auth, user_id)| {
            let alive = Rc::new(Cell::new(true));
            let items: Vec<PodrollItem> = podroll
                .iter()
                .filter(|item| {
                    !item.feed_guid.trim().is_empty() || nonempty(item.feed_url.as_deref()).is_some()
                })
                .cloned()
                .collect();
            shows.set(items.iter().map(placeholder).collect::<Vec<_>>());

            let alive_flag = alive.clone();
            let shows = shows.clone();
            let auth = auth.clone();
            let user_id = *user_id;
            spawn_local(async move {
                let Some((server, api_key)) = auth else {
                    return;
                };
                let mut resolved = Vec::with_capacity(items.len());
                for item in items {
                    let mut show = placeholder(&item);
                    let query = lookup_query(&item.feed_guid, item.feed_url.as_deref());
                    if !query.is_empty() {
                        if let Ok(result) =
                            call_get_podcast_info(&query, &server, &api_key, "podcast_index").await
                        {
                            let hits: Vec<IndexHit> = result
                                .feeds
                                .unwrap_or_default()
                                .iter()
                                .map(IndexHit::from_podcast)
                                .collect();
                            if let Some(hit) =
                                pick_index_hit(&hits, &item.feed_guid, item.feed_url.as_deref())
                            {
                                show = apply_hit(show, hit);
                            }
                        }
                    }
                    if user_id > 0 && !show.feed_url.is_empty() && !show.title.is_empty() {
                        if let Ok(check) = call_check_podcast(
                            &server,
                            &api_key,
                            user_id,
                            &show.title,
                            &show.feed_url,
                        )
                        .await
                        {
                            show.subscribed = check.exists;
                        }
                    }
                    resolved.push(show);
                }
                if !alive_flag.get() {
                    return;
                }
                shows.set({
                    let current = (*shows).clone();
                    for row in resolved.iter_mut() {
                        if let Some(prev) = current.iter().find(|prev| same_show(prev, row)) {
                            if prev.subscribed {
                                row.subscribed = true;
                            }
                            if prev.subscribing {
                                row.subscribing = true;
                            }
                        }
                    }
                    resolved
                });
            });

            move || {
                alive.set(false);
            }
        });
    }

    if props.podroll.is_empty() && props.value.is_empty() {
        return html! {};
    }

    let label_podroll = i18n.t("podcasting2.podroll").to_string();
    let label_hint = i18n.t("podcasting2.podroll_hint").to_string();
    let label_value = i18n.t("podcasting2.value_title").to_string();
    let label_supported = i18n.t("podcasting2.value_supported").to_string();
    let label_recipients = i18n.t("podcasting2.recipients").to_string();
    let label_suggested = i18n.t("podcasting2.suggested").to_string();
    let label_open = i18n.t("podcasting2.open").to_string();
    let label_subscribe = i18n.t("podcasting2.subscribe").to_string();
    let label_subscribed = i18n.t("podcasting2.subscribed").to_string();
    let label_subscribing = i18n.t("podcasting2.subscribing").to_string();
    let label_unresolved = i18n.t("podcasting2.unresolved").to_string();
    let label_display_only = i18n.t("podcasting2.display_only").to_string();
    let label_subscribe_ok = i18n.t("podcasting2.subscribe_ok").to_string();
    let label_subscribe_failed = i18n.t("podcasting2.subscribe_failed").to_string();

    let history = BrowserHistory::new();
    let server = auth.as_ref().map(|(server, _)| server.clone()).unwrap_or_default();
    let api_key = auth.as_ref().map(|(_, key)| key.clone()).unwrap_or_default();

    html! {
        <div class="p2-extras">
            { if props.value.is_empty() { html! {} } else { html! {
                <section class="p2-value">
                    <div class="p2-head">
                        <i class="ph ph-hand-heart" aria-hidden="true"></i>
                        <span class="p2-title">{ label_value }</span>
                    </div>
                    <p class="p2-note">{ label_supported }</p>
                    { for props.value.iter().map(|block| {
                        let method_line = method_line(block, &label_suggested);
                        html! {
                            <div class="p2-block">
                                { if method_line.is_empty() { html! {} } else { html! {
                                    <p class="p2-method">{ method_line }</p>
                                } } }
                                { if block.recipients.is_empty() { html! {} } else { html! {
                                    <>
                                        <p class="p2-recipients-label">{ label_recipients.clone() }</p>
                                        { for block.recipients.iter().map(|recipient| recipient_row(recipient)) }
                                    </>
                                } } }
                            </div>
                        }
                    }) }
                    <p class="p2-disclaimer">{ label_display_only }</p>
                </section>
            } } }
            { if shows.is_empty() { html! {} } else { html! {
                <section class="p2-podroll">
                    <div class="p2-head">
                        <i class="ph ph-broadcast" aria-hidden="true"></i>
                        <span class="p2-title">{ label_podroll }</span>
                    </div>
                    <p class="p2-note">{ label_hint }</p>
                    <div class="p2-row">
                        { for shows.iter().map(|show| {
                            let subscribe_label = if show.subscribing {
                                label_subscribing.clone()
                            } else if show.subscribed {
                                label_subscribed.clone()
                            } else {
                                label_subscribe.clone()
                            };
                            let can_open = !show.feed_url.is_empty() && user_id > 0 && !api_key.is_empty();
                            let on_open = if can_open {
                                let categories = if show.categories.is_empty() {
                                    None
                                } else {
                                    Some(show.categories.values().cloned().collect::<Vec<_>>().join(", "))
                                };
                                Some(create_on_title_click(
                                    server.clone(),
                                    Some(Some(api_key.clone())),
                                    &history,
                                    0,
                                    show.index_id,
                                    show.title.clone(),
                                    show.feed_url.clone(),
                                    show.description.clone(),
                                    show.author.clone(),
                                    if show.artwork.is_empty() {
                                        "/static/assets/favicon.png".to_string()
                                    } else {
                                        show.artwork.clone()
                                    },
                                    show.explicit,
                                    show.episode_count,
                                    categories,
                                    if show.link.is_empty() { show.feed_url.clone() } else { show.link.clone() },
                                    user_id,
                                    false,
                                ))
                            } else {
                                None
                            };
                            let on_subscribe = subscribe_click(
                                show,
                                server.clone(),
                                api_key.clone(),
                                user_id,
                                shows.clone(),
                                label_subscribe_ok.clone(),
                                label_subscribe_failed.clone(),
                            );
                            html! {
                                <article class="p2-card">
                                    { artwork(show) }
                                    <div class="p2-show-title">{ &show.title }</div>
                                    { if show.author.is_empty() { html! {} } else { html! {
                                        <div class="p2-show-author">{ &show.author }</div>
                                    } } }
                                    { if show.resolved { html! {} } else { html! {
                                        <div class="p2-show-author">{ &label_unresolved }</div>
                                    } } }
                                    <div class="p2-actions">
                                        <button
                                            type="button"
                                            class="p2-btn"
                                            title={label_open.clone()}
                                            disabled={!can_open}
                                            onclick={on_open.unwrap_or_else(|| Callback::from(|_| {}))}
                                        >
                                            { &label_open }
                                        </button>
                                        <button
                                            type="button"
                                            class="p2-btn p2-btn-primary"
                                            title={subscribe_label.clone()}
                                            disabled={show.feed_url.is_empty() || show.subscribed || show.subscribing || user_id == 0 || api_key.is_empty()}
                                            onclick={on_subscribe}
                                        >
                                            { subscribe_label }
                                        </button>
                                    </div>
                                </article>
                            }
                        }) }
                    </div>
                </section>
            } } }
        </div>
    }
}

fn method_line(block: &Value, suggested_label: &str) -> String {
    let mut parts = Vec::new();
    if !block.r#type.trim().is_empty() {
        parts.push(block.r#type.trim().to_string());
    }
    if !block.method.trim().is_empty() {
        parts.push(block.method.trim().to_string());
    }
    if let Some(suggested) = block.suggested.as_ref().map(|value| value.trim()).filter(|value| !value.is_empty()) {
        parts.push(format!("{suggested_label} {suggested}"));
    }
    parts.join(" · ")
}

fn recipient_row(recipient: &ValueRecipient) -> Html {
    let name = if recipient.name.trim().is_empty() {
        recipient.r#type.clone()
    } else {
        recipient.name.clone()
    };
    let split = format_split(&recipient.split);
    let address = recipient.address.trim().to_string();
    html! {
        <div class="p2-recipient">
            <span class="p2-recipient-name">{ name }</span>
            { if split.is_empty() { html! {} } else { html! {
                <span class="p2-split">{ split }</span>
            } } }
            { if address.is_empty() { html! {} } else { html! {
                <span class="p2-address" title={address.clone()}>{ address }</span>
            } } }
        </div>
    }
}

fn artwork(show: &ResolvedShow) -> Html {
    if show.artwork.trim().is_empty() {
        html! { <div class="p2-art p2-art-ph" aria-hidden="true"><i class="ph ph-microphone"></i></div> }
    } else {
        html! {
            <FallbackImage src={show.artwork.clone()} alt={show.title.clone()} class="p2-art" />
        }
    }
}

fn subscribe_click(
    show: &ResolvedShow,
    server: String,
    api_key: String,
    user_id: i32,
    shows: UseStateHandle<Vec<ResolvedShow>>,
    ok_msg: String,
    err_msg: String,
) -> Callback<MouseEvent> {
    let show = show.clone();
    Callback::from(move |_| {
        if show.feed_url.is_empty() || show.subscribed || show.subscribing || user_id == 0 || api_key.is_empty() {
            return;
        }
        let key = (show.feed_guid.clone(), show.feed_url.clone());
        set_flag(&shows, &key, true, show.subscribed);
        let shows = shows.clone();
        let show = show.clone();
        let server = server.clone();
        let api_key = Some(api_key.clone());
        let ok_msg = ok_msg.clone();
        let err_msg = err_msg.clone();
        spawn_local(async move {
            let values = PodcastValues {
                pod_title: show.title.clone(),
                pod_artwork: show.artwork.clone(),
                pod_author: show.author.clone(),
                categories: show.categories.clone(),
                pod_description: show.description.clone(),
                pod_episode_count: show.episode_count,
                pod_feed_url: show.feed_url.clone(),
                pod_website: if show.link.is_empty() { show.feed_url.clone() } else { show.link.clone() },
                pod_explicit: show.explicit,
                user_id,
            };
            let success = match call_add_podcast(&server, &api_key, user_id, &values, show.index_id).await {
                Ok(body) => body.success,
                Err(_) => false,
            };
            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                if success {
                    state.info_message = Some(ok_msg);
                } else {
                    state.error_message = Some(err_msg);
                }
            });
            set_flag(&shows, &key, false, success);
        });
    })
}

fn same_show(left: &ResolvedShow, right: &ResolvedShow) -> bool {
    if !left.feed_guid.is_empty() && left.feed_guid == right.feed_guid {
        return true;
    }
    !left.feed_url.is_empty() && left.feed_url == right.feed_url
}

fn set_flag(shows: &UseStateHandle<Vec<ResolvedShow>>, key: &(String, String), subscribing: bool, subscribed: bool) {
    let mut next = (**shows).clone();
    if let Some(row) = next.iter_mut().find(|row| {
        if !key.0.is_empty() {
            row.feed_guid == key.0
        } else {
            row.feed_url == key.1
        }
    }) {
        row.subscribing = subscribing;
        if subscribed {
            row.subscribed = true;
        }
    }
    shows.set(next);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::requests::pod_req::Podcasting2PodData;
    use wasm_bindgen_test::*;

    fn hit(guid: &str, url: &str, title: &str) -> IndexHit {
        IndexHit {
            guid: guid.to_string(),
            title: title.to_string(),
            url: url.to_string(),
            original_url: url.to_string(),
            artwork: String::new(),
            author: String::new(),
            description: String::new(),
            link: String::new(),
            id: 7,
            episode_count: 3,
            explicit: false,
            categories: HashMap::new(),
        }
    }

    #[test]
    fn format_split_appends_a_percent_once() {
        assert_eq!(format_split("90"), "90%");
        assert_eq!(format_split("90%"), "90%");
        assert_eq!(format_split(" 33.5 "), "33.5%");
        assert_eq!(format_split(""), "");
        assert_eq!(format_split("node"), "node");
    }

    #[test]
    fn lookup_query_prefers_a_feed_url() {
        assert_eq!(
            lookup_query("guid", Some(" https://example.com/rss ")),
            "https://example.com/rss"
        );
        assert_eq!(lookup_query("guid", Some("  ")), "guid");
        assert_eq!(lookup_query(" guid ", None), "guid");
    }

    #[test]
    fn pick_prefers_guid_over_a_url_collision() {
        let hits = vec![
            hit("other", "https://example.com/rss", "Wrong"),
            hit("GUID-1", "https://other.example/rss", "Right"),
        ];
        let picked = pick_index_hit(&hits, "guid-1", Some("https://example.com/rss")).unwrap();
        assert_eq!(picked.title, "Right");
    }

    #[test]
    fn pick_matches_feed_url_and_a_single_url_search_result() {
        let hits = vec![hit("", "https://example.com/rss/", "By URL")];
        let picked = pick_index_hit(&hits, "missing", Some("https://example.com/rss")).unwrap();
        assert_eq!(picked.title, "By URL");

        let loose = vec![hit("", "https://a.example/rss", "A"), hit("", "https://b.example/rss", "B")];
        assert!(pick_index_hit(&loose, "missing-guid", None).is_none());
    }

    #[test]
    fn titles_fall_back_from_name_to_host_to_guid() {
        assert_eq!(podroll_title(Some("Show"), Some("https://ex.com/rss"), "g"), "Show");
        assert_eq!(podroll_title(None, Some("https://feeds.ex.com/rss"), "g"), "feeds.ex.com");
        assert_eq!(podroll_title(None, None, "1234567890abcdef"), "12345678…");
    }

    #[test]
    fn channel_payload_parses_podroll_and_value() {
        let raw = r#"{
            "people": [],
            "guests": [],
            "podroll": [
                {"feed_guid": "g-1"},
                {"feedGuid": "g-2", "feedUrl": "https://example.com/rss", "title": "Friend"}
            ],
            "funding": [],
            "value": [{
                "type": "lightning",
                "method": "keysend",
                "suggested": "5000",
                "recipients": [
                    {"name": "Alice", "type": "node", "address": "03ab", "split": "90"},
                    {"name": "App", "type": "node", "address": "02cd", "split": "10"}
                ]
            }]
        }"#;
        let data: Podcasting2PodData = serde_json::from_str(raw).unwrap();
        assert_eq!(data.podroll[0].feed_guid, "g-1");
        assert!(data.podroll[0].feed_url.is_none());
        assert_eq!(data.podroll[1].feed_url.as_deref(), Some("https://example.com/rss"));
        assert_eq!(data.podroll[1].title.as_deref(), Some("Friend"));
        assert_eq!(data.value[0].r#type, "lightning");
        assert_eq!(format_split(&data.value[0].recipients[0].split), "90%");
        assert_eq!(format_split(&data.value[0].recipients[1].split), "10%");
    }

    /// The Podcasting 2.0 backend adds `guid` and `tags` and nests extra fields on the
    /// same podroll and value objects. Those keys stay readable.
    #[wasm_bindgen_test]
    fn channel_payload_with_namespace_tags_still_reads_podroll_and_value() {
        let raw = r#"{
            "people": [],
            "guests": [],
            "podroll": [{
                "feed_guid": "29cdca4a-32d8-56ba-b48b-09a011c5daa9",
                "feed_url": "https://example.com/rss",
                "item_guid": "ep-1",
                "medium": "podcast",
                "title": "Friend"
            }],
            "funding": [{"url": "https://fund.example", "description": "Support us"}],
            "value": [{
                "type": "lightning",
                "method": "keysend",
                "suggested": null,
                "recipients": [{
                    "name": "Alice",
                    "type": "node",
                    "address": "03ab",
                    "split": "90",
                    "custom_key": "696969",
                    "custom_value": "x",
                    "fee": false
                }],
                "time_splits": [{"start_time": "0", "duration": "10", "recipients": []}]
            }],
            "guid": "917393e3-1b1e-5cef-ace4-edaa54e1f810",
            "tags": {
                "medium": "music",
                "podroll": [{"feed_guid": "29cdca4a-32d8-56ba-b48b-09a011c5daa9"}],
                "value": [{"type": "lightning", "method": "keysend", "suggested": null, "recipients": []}]
            }
        }"#;
        let data: Podcasting2PodData = serde_json::from_str(raw).unwrap();
        assert_eq!(data.podroll[0].feed_guid, "29cdca4a-32d8-56ba-b48b-09a011c5daa9");
        assert_eq!(data.podroll[0].feed_url.as_deref(), Some("https://example.com/rss"));
        assert_eq!(data.podroll[0].title.as_deref(), Some("Friend"));
        assert!(data.podroll[0].artwork_url.is_none());
        assert_eq!(data.funding[0].description, "Support us");
        assert_eq!(data.value[0].r#type, "lightning");
        assert_eq!(data.value[0].method, "keysend");
        assert!(data.value[0].suggested.is_none());
        assert_eq!(data.value[0].recipients[0].name, "Alice");
        assert_eq!(data.value[0].recipients[0].r#type, "node");
        assert_eq!(format_split(&data.value[0].recipients[0].split), "90%");
    }
}

fn t_replace(template: String, pairs: &[(&str, &str)]) -> String {
    pairs.iter().fold(template, |acc, (key, value)| acc.replace(key, value))
}

fn preview_text(text: &str) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out = String::new();
    for (index, ch) in flat.chars().enumerate() {
        if index == 277 {
            out.push_str("...");
            break;
        }
        out.push(ch);
    }
    out
}

fn medium_label(i18n: &i18nrs::I18n, medium: &str) -> String {
    match medium.trim().to_ascii_lowercase().as_str() {
        "podcast" => i18n.t("podcasting2.medium_podcast").to_string(),
        "music" => i18n.t("podcasting2.medium_music").to_string(),
        "video" => i18n.t("podcasting2.medium_video").to_string(),
        "film" => i18n.t("podcasting2.medium_film").to_string(),
        "audiobook" => i18n.t("podcasting2.medium_audiobook").to_string(),
        "newsletter" => i18n.t("podcasting2.medium_newsletter").to_string(),
        "blog" => i18n.t("podcasting2.medium_blog").to_string(),
        "publisher" => i18n.t("podcasting2.medium_publisher").to_string(),
        _ => medium.trim().to_string(),
    }
}

fn cadence_label(i18n: &i18nrs::I18n, cadence: &Cadence) -> String {
    let (n, one, many) = match cadence {
        Cadence::Hourly(n) => (*n, "podcasting2.unit_hour", "podcasting2.unit_hours"),
        Cadence::Daily(n) => (*n, "podcasting2.unit_day", "podcasting2.unit_days"),
        Cadence::Weekly(n) => (*n, "podcasting2.unit_week", "podcasting2.unit_weeks"),
        Cadence::Monthly(n) => (*n, "podcasting2.unit_month", "podcasting2.unit_months"),
        Cadence::Yearly(n) => (*n, "podcasting2.unit_year", "podcasting2.unit_years"),
    };
    let unit = if n == 1 { i18n.t(one).to_string() } else { i18n.t(many).to_string() };
    if n == 1 {
        t_replace(i18n.t("podcasting2.every_one").to_string(), &[("{unit}", &unit)])
    } else {
        t_replace(
            i18n.t("podcasting2.every_n").to_string(),
            &[("{n}", &n.to_string()), ("{unit}", &unit)],
        )
    }
}

/// Translated season/episode line. Empty when neither Podcasting 2.0 nor iTunes has a number.
pub fn episode_numbers_text(
    i18n: &i18nrs::I18n,
    season: Option<&Season>,
    episode: Option<&EpisodeNumber>,
    itunes_season: Option<i64>,
    itunes_episode: Option<i64>,
) -> Option<String> {
    let numbers = resolve_numbers(season, episode, itunes_season, itunes_episode);
    format_numbers_badge(
        &i18n.t("podcasting2.season"),
        &i18n.t("podcasting2.episode"),
        &numbers,
    )
}

fn section(title: String, body: Html) -> Html {
    html! {
        <section class="p2-card">
            <h3 class="p2-card-title">{ title }</h3>
            { body }
        </section>
    }
}

fn external_link(href: String, text: String) -> Html {
    html! {
        <a class="p2-link" href={href} target="_blank" rel="noopener noreferrer">{ text }</a>
    }
}

fn pause_player(state: &mut UIState) {
    if let Some(media) = state.media_element.as_ref() {
        let _ = media.pause();
    } else if let Some(audio) = state.audio_element.as_ref() {
        let _ = audio.pause();
    }
    state.audio_playing = Some(false);
}

fn seek_player(state: &mut UIState, at: f64) {
    let hours = (at / 3600.0).floor() as i32;
    let minutes = ((at % 3600.0) / 60.0).floor() as i32;
    let seconds = (at % 60.0).floor() as i32;
    let formatted = format!("{hours:02}:{minutes:02}:{seconds:02}");
    if let Some(media) = state.media_element.as_ref() {
        media.set_current_time(at);
        let _ = media.play();
    } else if let Some(audio) = state.audio_element.as_ref() {
        audio.set_current_time(at);
        let _ = audio.play();
    }
    state.current_time_seconds = at;
    state.current_time_formatted = formatted;
    state.audio_playing = Some(true);
}

#[derive(Properties, PartialEq, Clone)]
pub struct ResponsiveArtworkProps {
    pub src: String,
    pub srcset: Option<String>,
    pub alt: String,
    pub class: String,
}

/// Cover art that uses a `srcset` when the feed published one.
#[function_component(ResponsiveArtwork)]
pub fn responsive_artwork(props: &ResponsiveArtworkProps) -> Html {
    let failed = use_state(|| false);
    let src = props.src.clone();
    let onerror = {
        let failed = failed.clone();
        Callback::from(move |_| failed.set(true))
    };
    if *failed || props.srcset.is_none() {
        html! { <img src={src} alt={props.alt.clone()} class={props.class.clone()} onerror={onerror} /> }
    } else {
        html! {
            <img
                src={src}
                srcset={props.srcset.clone().unwrap_or_default()}
                alt={props.alt.clone()}
                class={props.class.clone()}
                onerror={onerror}
            />
        }
    }
}

#[derive(Properties, PartialEq, Clone)]
pub struct EpisodeTagsPanelProps {
    pub tags: ItemTags,
    pub itunes_season: Option<i64>,
    pub itunes_episode: Option<i64>,
    pub episode: Episode,
    pub api_key: String,
    pub user_id: i32,
    pub server_name: String,
}

#[function_component(EpisodeTagsPanel)]
pub fn episode_tags_panel(props: &EpisodeTagsPanelProps) -> Html {
    let (i18n, _) = use_translation();
    let (ui, dispatch) = use_store::<UIState>();
    let numbers = resolve_numbers(
        props.tags.season.as_ref(),
        props.tags.episode.as_ref(),
        props.itunes_season,
        props.itunes_episode,
    );
    let badge = format_numbers_badge(&i18n.t("podcasting2.season"), &i18n.t("podcasting2.episode"), &numbers);

    {
        let dispatch = dispatch.clone();
        let episode_id = props.episode.episodeid;
        let stop = ui.soundbite_stop_at;
        let now = ui.current_time_seconds;
        let playing_id = ui.currently_playing.as_ref().map(|c| c.episode_id);
        use_effect_with((stop, now as i32, playing_id), move |_| {
            if let Some(end) = stop {
                if playing_id == Some(episode_id) && now + 0.25 >= end {
                    dispatch.reduce_mut(|state| {
                        pause_player(state);
                        state.soundbite_stop_at = None;
                    });
                }
            }
            || ()
        });
    }

    let links: Vec<(String, String)> = props
        .tags
        .content_links
        .iter()
        .filter_map(|link| {
            safe_link(&link.href).map(|href| {
                let text = if link.text.trim().is_empty() { href.to_string() } else { link.text.clone() };
                (href.to_string(), text)
            })
        })
        .collect();
    let socials: Vec<(String, String)> = props
        .tags
        .social_interacts
        .iter()
        .filter_map(|social| {
            social_url(social).map(|url| {
                let label = if social.protocol.trim().is_empty() {
                    i18n.t("podcasting2.comment_thread").to_string()
                } else {
                    social.protocol.clone()
                };
                (url, label)
            })
        })
        .collect();
    let splits = time_splits_of(&props.tags.value);
    let chat = props.tags.chat.as_ref().and_then(chat_url);
    let license_url = props.tags.license.as_ref().and_then(|l| l.url.as_deref().and_then(http_link).map(|s| s.to_string()));
    let has_license = props.tags.license.as_ref().map(|l| !l.identifier.trim().is_empty() || l.url.is_some()).unwrap_or(false);
    let medium = props.tags.medium.as_deref().map(str::trim).filter(|m| !m.is_empty()).map(|m| m.to_string());

    let empty = badge.is_none()
        && props.tags.soundbites.is_empty()
        && links.is_empty()
        && props.tags.locations.is_empty()
        && chat.is_none()
        && socials.is_empty()
        && !has_license
        && medium.is_none()
        && props.tags.remote_items.is_empty()
        && splits.is_empty()
        && props.tags.funding.is_empty();
    if empty {
        return html! {};
    }

    html! {
        <div class="p2-block">
            { if let Some(badge) = badge.clone() {
                html! { <p class="p2-badge">{ badge }</p> }
            } else { html! {} } }
            { if let Some(medium) = medium.as_deref() {
                section(i18n.t("podcasting2.medium").to_string(), html! { <p>{ medium_label(&i18n, medium) }</p> })
            } else { html! {} } }
            { if !props.tags.soundbites.is_empty() {
                let rows = props.tags.soundbites.iter().enumerate().filter_map(|(index, bite)| {
                    let window = soundbite_window(&bite.start_time, &bite.duration)?;
                    let title = bite.title.clone().filter(|t| !t.trim().is_empty()).unwrap_or_else(|| {
                        t_replace(i18n.t("podcasting2.clip_n").to_string(), &[("{n}", &(index + 1).to_string())])
                    });
                    let playing = ui.soundbite_stop_at.map(|end| (end - window.end).abs() < 0.5).unwrap_or(false)
                        && ui.audio_playing.unwrap_or(false)
                        && ui.currently_playing.as_ref().map(|c| c.episode_id) == Some(props.episode.episodeid);
                    let dispatch = dispatch.clone();
                    let episode = props.episode.clone();
                    let api_key = props.api_key.clone();
                    let server_name = props.server_name.clone();
                    let user_id = props.user_id;
                    let onclick = Callback::from(move |e: MouseEvent| {
                        if playing {
                            dispatch.reduce_mut(|state| {
                                pause_player(state);
                                state.soundbite_stop_at = None;
                            });
                            return;
                        }
                        let loaded = Dispatch::<UIState>::global().get().currently_playing.as_ref().map(|c| c.episode_id) == Some(episode.episodeid);
                        if loaded {
                            dispatch.reduce_mut(|state| {
                                seek_player(state, window.start);
                                if window.end > window.start {
                                    state.soundbite_stop_at = Some(window.end);
                                }
                            });
                        } else {
                            let mut ep = episode.clone();
                            ep.listenduration = window.start.floor() as i32;
                            if window.end > window.start {
                                dispatch.reduce_mut(|state| state.soundbite_stop_at = Some(window.end));
                            }
                            on_play_click(ep, api_key.clone(), user_id, server_name.clone(), dispatch.clone(), Dispatch::<UIState>::global().get(), false, false, None).emit(e);
                        }
                    });
                    Some(html! {
                        <li class="p2-row">
                            <button class="p2-play" onclick={onclick} title={if playing { i18n.t("podcasting2.stop_clip").to_string() } else { i18n.t("podcasting2.play_clip").to_string() }}>
                                <i class={classes!("ph", if playing { "ph-pause" } else { "ph-play" })}></i>
                            </button>
                            <span class="p2-row-title">{ title }</span>
                            <span class="p2-row-meta">{ format!("{} · {}", format_clock(window.start), format_clock(window.end - window.start)) }</span>
                        </li>
                    })
                }).collect::<Html>();
                section(i18n.t("podcasting2.soundbites").to_string(), html! { <ul class="p2-list">{ rows }</ul> })
            } else { html! {} } }
            { if !links.is_empty() {
                section(i18n.t("podcasting2.content_links").to_string(), html! {
                    <ul class="p2-list">
                        { for links.iter().map(|(href, text)| html! {
                            <li class="p2-row">{ external_link(href.clone(), text.clone()) }</li>
                        }) }
                    </ul>
                })
            } else { html! {} } }
            { if !props.tags.locations.is_empty() {
                section(i18n.t("podcasting2.location").to_string(), html! {
                    <ul class="p2-list">
                        { for props.tags.locations.iter().map(|location| {
                            let map = location_map_url(location);
                            let mut detail = location.name.clone();
                            if let Some(country) = location.country.as_deref().filter(|c| !c.is_empty()) {
                                detail = format!("{detail} · {country}");
                            }
                            html! {
                                <li class="p2-row">
                                    <span class="p2-row-title">{ detail }</span>
                                    { if let Some(map) = map {
                                        external_link(map, i18n.t("podcasting2.open_map").to_string())
                                    } else { html! {} } }
                                </li>
                            }
                        }) }
                    </ul>
                })
            } else { html! {} } }
            { if let Some(chat) = chat {
                section(i18n.t("podcasting2.chat").to_string(), html! {
                    { external_link(chat, i18n.t("podcasting2.join_chat").to_string()) }
                })
            } else { html! {} } }
            { if !socials.is_empty() {
                section(i18n.t("podcasting2.comments").to_string(), html! {
                    <ul class="p2-list">
                        { for socials.iter().map(|(url, label)| html! {
                            <li class="p2-row">{ external_link(url.clone(), label.clone()) }</li>
                        }) }
                    </ul>
                })
            } else { html! {} } }
            { if has_license {
                let ident = props.tags.license.as_ref().map(|l| l.identifier.clone()).unwrap_or_default();
                section(i18n.t("podcasting2.license").to_string(), html! {
                    <>
                        { if ident.trim().is_empty() { html! {} } else { html! { <span>{ ident }</span> } } }
                        { if let Some(url) = license_url.clone() {
                            html! { <>{" "}{ external_link(url, i18n.t("podcasting2.open_license").to_string()) }</> }
                        } else { html! {} } }
                    </>
                })
            } else { html! {} } }
            { if !props.tags.funding.is_empty() {
                section(i18n.t("podcasting2.funding").to_string(), html! {
                    <ul class="p2-list">
                        { for props.tags.funding.iter().filter_map(|funding| http_link(&funding.url).map(|url| {
                            let text = if funding.description.trim().is_empty() { url.to_string() } else { funding.description.clone() };
                            html! { <li class="p2-row">{ external_link(url.to_string(), text) }</li> }
                        })) }
                    </ul>
                })
            } else { html! {} } }
            { if !props.tags.remote_items.is_empty() {
                section(i18n.t("podcasting2.related").to_string(), html! { <ul class="p2-list">{ remote_rows(&props.tags.remote_items, &i18n) }</ul> })
            } else { html! {} } }
            { if !splits.is_empty() {
                section(i18n.t("podcasting2.time_splits").to_string(), html! {
                    <ul class="p2-list">
                        { for splits.iter().map(|split| {
                            let start = parse_clock(&split.start_time);
                            let duration = parse_clock(&split.duration);
                            let names: Vec<&str> = split.recipients.iter().map(|r| r.name.as_str()).filter(|n| !n.is_empty()).collect();
                            let remote = split.remote_item.as_ref().and_then(|r| r.title.clone().or(r.feed_url.clone()));
                            html! {
                                <li class="p2-row">
                                    <span class="p2-row-title">{ format!("{} · {}", start, duration) }</span>
                                    { if names.is_empty() { html! {} } else { html! { <span class="p2-row-meta">{ names.join(", ") }</span> } } }
                                    { if let Some(remote) = remote { html! { <span class="p2-row-meta">{ remote }</span> } } else { html! {} } }
                                    { if let Some(pct) = split.remote_percentage.as_deref().filter(|p| !p.is_empty()) {
                                        html! { <span class="p2-row-meta">{ t_replace(i18n.t("podcasting2.remote_percent").to_string(), &[("{pct}", pct)]) }</span> }
                                    } else { html! {} } }
                                </li>
                            }
                        }) }
                    </ul>
                })
            } else { html! {} } }
        </div>
    }
}

fn parse_clock(raw: &str) -> String {
    match raw.trim().parse::<f64>() {
        Ok(n) if n.is_finite() => format_clock(n),
        _ => raw.trim().to_string(),
    }
}

fn remote_rows(items: &[RemoteItem], i18n: &i18nrs::I18n) -> Html {
    items.iter().filter(|item| !item.feed_guid.is_empty() || item.title.is_some() || item.feed_url.is_some()).map(|item| {
        let title = item.title.clone().filter(|t| !t.trim().is_empty()).unwrap_or_else(|| {
            item.medium.clone().unwrap_or_else(|| i18n.t("podcasting2.related").to_string())
        });
        let medium = item.medium.as_deref().map(|m| medium_label(i18n, m));
        html! {
            <li class="p2-row">
                <span class="p2-row-title">{ title }</span>
                { if let Some(medium) = medium { html! { <span class="p2-row-meta">{ medium }</span> } } else { html! {} } }
                { if let Some(url) = item.feed_url.as_deref().and_then(http_link) {
                    external_link(url.to_string(), i18n.t("podcasting2.open_feed").to_string())
                } else { html! {} } }
            </li>
        }
    }).collect::<Html>()
}

#[derive(Properties, PartialEq, Clone)]
pub struct ShowTagsPanelProps {
    pub tags: ChannelTags,
    /// Guids of episodes already listed for this show. An ended live item with one of these is hidden.
    #[prop_or_default]
    pub episode_guids: Vec<String>,
}

#[function_component(ShowTagsPanel)]
pub fn show_tags_panel(props: &ShowTagsPanelProps) -> Html {
    let (i18n, _) = use_translation();
    let playing: UseStateHandle<Option<String>> = use_state(|| None);
    let audio_ref = use_node_ref();
    let video_ref = use_node_ref();

    {
        let playing_url = (*playing).clone();
        let audio_ref = audio_ref.clone();
        let video_ref = video_ref.clone();
        use_effect_with(playing_url.clone(), move |url| {
            if url.is_some() {
                if let Some(el) = video_ref.cast::<HtmlVideoElement>() {
                    let _ = el.play();
                } else if let Some(el) = audio_ref.cast::<HtmlAudioElement>() {
                    let _ = el.play();
                }
            }
            || ()
        });
    }

    let tags = &props.tags;
    let medium = tags.medium.as_deref().map(str::trim).filter(|m| !m.is_empty()).map(|m| m.to_string());
    let schedule_text = tags.update_frequency.as_ref().and_then(|freq| {
        let text = freq.text.trim();
        let cadence = freq.rrule.as_deref().and_then(cadence_of).map(|c| cadence_label(&i18n, &c));
        let complete = freq.complete.unwrap_or(false);
        if text.is_empty() && cadence.is_none() && !complete {
            None
        } else {
            Some((text.to_string(), cadence, complete))
        }
    });
    let chat = tags.chat.as_ref().and_then(chat_url);
    let socials: Vec<(String, String)> = tags.social_interacts.iter().filter_map(|social| {
        social_url(social).map(|url| {
            let label = if social.protocol.trim().is_empty() {
                i18n.t("podcasting2.comment_thread").to_string()
            } else {
                social.protocol.clone()
            };
            (url, label)
        })
    }).collect();
    let has_license = tags.license.as_ref().map(|l| !l.identifier.trim().is_empty() || l.url.is_some()).unwrap_or(false);
    let groups = trailer_groups(&tags.trailers);
    let show_season_headers = groups.iter().any(|(season, _)| season.is_some());

    let live_items = visible_live_items(&tags.live_items, &props.episode_guids);
    let artwork = tags
        .image
        .iter()
        .find_map(|img| http_link(&img.href).map(|url| url.to_string()))
        .unwrap_or_default();
    let empty = medium.is_none()
        && schedule_text.is_none()
        && tags.trailers.is_empty()
        && tags.publisher.is_none()
        && tags.locations.is_empty()
        && chat.is_none()
        && socials.is_empty()
        && !has_license
        && tags.remote_items.is_empty()
        && live_items.is_empty();
    if empty {
        return html! {};
    }

    html! {
        <div class="p2-block">
            { if !live_items.is_empty() {
                section(i18n.t("podcasting2.live_items").to_string(), html! {
                    <ul class="p2-list">
                        { for live_items.iter().map(|item| {
                            let status = normalize_live_status(&item.status);
                            let status_label = match status {
                                "live" => i18n.t("podcasting2.live_now").to_string(),
                                "ended" => i18n.t("podcasting2.ended").to_string(),
                                _ => i18n.t("podcasting2.upcoming").to_string(),
                            };
                            let when = item.start.trim();
                            let starts = if when.is_empty() {
                                String::new()
                            } else {
                                t_replace(i18n.t("podcasting2.starts").to_string(), &[("{when}", when)])
                            };
                            let title = item.title.as_deref().map(str::trim).filter(|title| !title.is_empty()).unwrap_or(status_label.as_str()).to_string();
                            let description = item.description.as_deref().map(str::trim).filter(|text| !text.is_empty()).map(preview_text);
                            let page = item.link.as_deref().and_then(http_link).map(|url| url.to_string());
                            let playback = item.tags.enclosure_url.as_deref().and_then(|url| live_playback(item.enclosure_type.as_deref(), url));
                            let play_label = if status == "ended" {
                                i18n.t("podcasting2.play_recording").to_string()
                            } else {
                                i18n.t("podcasting2.play_live").to_string()
                            };
                            let playing_state = playing.clone();
                            let audio_ref = audio_ref.clone();
                            let video_ref = video_ref.clone();
                            let play_title = title.clone();
                            let play_description = description.clone().unwrap_or_default();
                            let play_artwork = artwork.clone();
                            let play_start = item.start.clone();
                            let onclick = playback.as_ref().map(|stream| {
                                let stream = stream.clone();
                                Callback::from(move |_| {
                                    playing_state.set(None);
                                    if let Some(el) = audio_ref.cast::<HtmlAudioElement>() {
                                        let _ = el.pause();
                                    }
                                    if let Some(el) = video_ref.cast::<HtmlVideoElement>() {
                                        let _ = el.pause();
                                    }
                                    play_live_stream(
                                        play_title.clone(),
                                        play_description.clone(),
                                        stream.url.clone(),
                                        play_artwork.clone(),
                                        stream.hls,
                                        stream.is_video,
                                        play_start.clone(),
                                        Dispatch::<UIState>::global(),
                                    );
                                })
                            });
                            html! {
                                <li class="p2-row">
                                    { if let Some(onclick) = onclick {
                                        html! {
                                            <button class="p2-play" onclick={onclick} title={play_label.clone()}>
                                                <i class="ph ph-play"></i>
                                            </button>
                                        }
                                    } else { html! {} } }
                                    <span class="p2-row-title">{ title }</span>
                                    <span class="p2-badge-mini">{ status_label }</span>
                                    { if starts.is_empty() { html! {} } else { html! { <span class="p2-row-meta">{ starts }</span> } } }
                                    { if let Some(page) = page {
                                        html! { { external_link(page, i18n.t("podcasting2.open").to_string()) } }
                                    } else { html! {} } }
                                    { if let Some(description) = description {
                                        html! { <p class="p2-row-meta">{ description }</p> }
                                    } else { html! {} } }
                                </li>
                            }
                        }) }
                    </ul>
                })
            } else { html! {} } }
            { if let Some(medium) = medium.as_deref() {
                section(i18n.t("podcasting2.medium").to_string(), html! { <p>{ medium_label(&i18n, medium) }</p> })
            } else { html! {} } }
            { if let Some((text, cadence, complete)) = schedule_text.clone() {
                section(i18n.t("podcasting2.schedule").to_string(), html! {
                    <>
                        { if text.is_empty() { html! {} } else { html! { <p>{ text }</p> } } }
                        { if let Some(cadence) = cadence { html! { <p class="p2-row-meta">{ cadence }</p> } } else { html! {} } }
                        { if complete { html! { <p class="p2-row-meta">{ i18n.t("podcasting2.schedule_complete") }</p> } } else { html! {} } }
                    </>
                })
            } else { html! {} } }
            { if !tags.trailers.is_empty() {
                section(i18n.t("podcasting2.trailers").to_string(), html! {
                    <>
                        { for groups.iter().map(|(season, indexes)| {
                            html! {
                                <div class="p2-trailer-group">
                                    { if show_season_headers {
                                        if let Some(season) = season {
                                            html! { <h4 class="p2-group">{ t_replace(i18n.t("podcasting2.season_group").to_string(), &[("{n}", season)]) }</h4> }
                                        } else {
                                            html! { <h4 class="p2-group">{ i18n.t("podcasting2.other_episodes") }</h4> }
                                        }
                                    } else { html! {} } }
                                    <ul class="p2-list">
                                        { for indexes.iter().filter_map(|index| tags.trailers.get(*index).map(|trailer| {
                                            let url = trailer.url.clone();
                                            let is_video = trailer_is_video(trailer.mime_type.as_deref());
                                            let active = (*playing).as_deref() == Some(url.as_str());
                                            let playing_state = playing.clone();
                                            let audio_dispatch = Dispatch::<UIState>::global();
                                            let onclick = Callback::from(move |_| {
                                                if active {
                                                    playing_state.set(None);
                                                } else {
                                                    audio_dispatch.reduce_mut(pause_player);
                                                    playing_state.set(Some(url.clone()));
                                                }
                                            });
                                            let title = if trailer.title.trim().is_empty() { i18n.t("podcasting2.trailer").to_string() } else { trailer.title.clone() };
                                            html! {
                                                <li class="p2-row p2-trailer">
                                                    <button class="p2-play" onclick={onclick} title={i18n.t("podcasting2.play_trailer").to_string()}>
                                                        <i class={classes!("ph", if active { "ph-pause" } else { "ph-play" })}></i>
                                                    </button>
                                                    <span class="p2-row-title">{ title }</span>
                                                    <span class="p2-badge-mini">{ i18n.t("podcasting2.trailer") }</span>
                                                    { if active && is_video {
                                                        html! { <video ref={video_ref.clone()} class="p2-trailer-video" src={trailer.url.clone()} controls=true /> }
                                                    } else if active {
                                                        html! { <audio ref={audio_ref.clone()} src={trailer.url.clone()} controls=true /> }
                                                    } else { html! {} } }
                                                </li>
                                            }
                                        })) }
                                    </ul>
                                </div>
                            }
                        }) }
                    </>
                })
            } else { html! {} } }
            { if let Some(publisher) = tags.publisher.as_ref() {
                section(i18n.t("podcasting2.publisher").to_string(), html! { <ul class="p2-list">{ remote_rows(std::slice::from_ref(publisher), &i18n) }</ul> })
            } else { html! {} } }
            { if !tags.locations.is_empty() {
                section(i18n.t("podcasting2.location").to_string(), html! {
                    <ul class="p2-list">
                        { for tags.locations.iter().map(|location| {
                            let map = location_map_url(location);
                            html! {
                                <li class="p2-row">
                                    <span class="p2-row-title">{ location.name.clone() }</span>
                                    { if let Some(map) = map { external_link(map, i18n.t("podcasting2.open_map").to_string()) } else { html! {} } }
                                </li>
                            }
                        }) }
                    </ul>
                })
            } else { html! {} } }
            { if let Some(chat) = chat {
                section(i18n.t("podcasting2.chat").to_string(), html! { { external_link(chat, i18n.t("podcasting2.join_chat").to_string()) } })
            } else { html! {} } }
            { if !socials.is_empty() {
                section(i18n.t("podcasting2.comments").to_string(), html! {
                    <ul class="p2-list">
                        { for socials.iter().map(|(url, label)| html! { <li class="p2-row">{ external_link(url.clone(), label.clone()) }</li> }) }
                    </ul>
                })
            } else { html! {} } }
            { if has_license {
                let ident = tags.license.as_ref().map(|l| l.identifier.clone()).unwrap_or_default();
                let url = tags.license.as_ref().and_then(|l| l.url.as_deref().and_then(http_link).map(|s| s.to_string()));
                section(i18n.t("podcasting2.license").to_string(), html! {
                    <>
                        { if ident.trim().is_empty() { html! {} } else { html! { <span>{ ident }</span> } } }
                        { if let Some(url) = url { html! { <>{" "}{ external_link(url, i18n.t("podcasting2.open_license").to_string()) }</> } } else { html! {} } }
                    </>
                })
            } else { html! {} } }
            { if !tags.remote_items.is_empty() {
                section(i18n.t("podcasting2.related").to_string(), html! { <ul class="p2-list">{ remote_rows(&tags.remote_items, &i18n) }</ul> })
            } else { html! {} } }
        </div>
    }
}

/// Cover src and srcset for a channel or episode, falling back to the artwork the page already has.
pub fn cover_art(images: &[PodcastImage], set: Option<&ImageSet>, fallback: &str) -> ArtworkChoice {
    artwork_choice(images, set, fallback)
}

pub fn cover_html(choice: &ArtworkChoice, alt: String, class: String) -> Html {
    if choice.srcset.is_some() {
        html! {
            <ResponsiveArtwork
                src={choice.src.clone()}
                srcset={choice.srcset.clone()}
                {alt}
                {class}
            />
        }
    } else {
        html! { <img src={choice.src.clone()} {alt} {class} /> }
    }
}
