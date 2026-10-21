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
}
