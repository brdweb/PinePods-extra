//! Podcasting 2.0 details that sit beside the episode and show pages.
//!
//! Playback of a soundbite seeks the episode already loaded in the player. Trailers use their
//! own audio or video element so they don't replace the episode.

use i18nrs::yew::use_translation;
use web_sys::{HtmlAudioElement, HtmlVideoElement};
use yew::prelude::*;
use yewdux::prelude::*;

use crate::components::audio::on_play_click;
use crate::components::context::UIState;
use crate::requests::episode::Episode;
use crate::requests::podcasting2::*;

fn t_replace(template: String, pairs: &[(&str, &str)]) -> String {
    pairs.iter().fold(template, |acc, (key, value)| acc.replace(key, value))
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

    let empty = medium.is_none()
        && schedule_text.is_none()
        && tags.trailers.is_empty()
        && tags.publisher.is_none()
        && tags.locations.is_empty()
        && chat.is_none()
        && socials.is_empty()
        && !has_license
        && tags.remote_items.is_empty();
    if empty {
        return html! {};
    }

    html! {
        <div class="p2-block">
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
