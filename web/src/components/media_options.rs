//! Version picker for the RSS enclosure and `podcast:alternateEnclosure`.
//!
//! The server plans playback and downloads. This module remembers the user's
//! choice in localStorage (same key as the mobile app) and restarts playback
//! when they pick a different playable version.

use crate::components::context::UIState;
use crate::requests::pod_req::{MediaOption, MediaPlan, PlaybackStep};
use i18nrs::yew::use_translation;
use wasm_bindgen::JsCast;
use web_sys::{window, Event, HtmlSelectElement, MouseEvent};
use yew::prelude::*;
use yewdux::prelude::*;

const PREF_PREFIX: &str = "pinepods.mediaOption.";

pub fn preferred_media_option(episode_id: i32) -> Option<String> {
    let storage = window()?.local_storage().ok().flatten()?;
    let value = storage
        .get_item(&format!("{PREF_PREFIX}{episode_id}"))
        .ok()
        .flatten()?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub fn store_preferred_media_option(episode_id: i32, option_id: &str) {
    let Some(storage) = window().and_then(|w| w.local_storage().ok().flatten()) else {
        return;
    };
    let _ = storage.set_item(&format!("{PREF_PREFIX}{episode_id}"), option_id);
}

/// Selected version's sources first. The rest keep their previous order.
pub fn reorder_playback(steps: &[PlaybackStep], selected_id: &str) -> Vec<PlaybackStep> {
    let mut ordered = steps.to_vec();
    ordered.sort_by_key(|step| if step.option_id == selected_id { 0 } else { 1 });
    ordered
}

pub fn render_media_picker(plan: Option<MediaPlan>, episode_id: i32) -> Html {
    match plan {
        Some(plan) if plan.options.len() > 1 && episode_id != 0 => html! {
            <MediaVersionPicker plan={plan} episode_id={episode_id} />
        },
        _ => html! {},
    }
}

fn option_label(
    option: &MediaOption,
    original: &str,
    audio: &str,
    video: &str,
    hls: &str,
    dash: &str,
    other: &str,
    kbps_template: &str,
    ipfs: &str,
    torrent: &str,
    cannot_play: &str,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    if option.id == "primary" {
        parts.push(original.to_string());
    } else if let Some(title) = option.title.as_ref().map(|t| t.trim()).filter(|t| !t.is_empty()) {
        parts.push(title.to_string());
    } else {
        parts.push(
            match option.kind.as_str() {
                "video" => video,
                "hls" => hls,
                "dash" => dash,
                "audio" => audio,
                _ => other,
            }
            .to_string(),
        );
    }
    if let Some(bitrate) = option.bitrate.filter(|rate| *rate > 0) {
        let kbps = if bitrate >= 1000 { bitrate / 1000 } else { bitrate };
        parts.push(kbps_template.replace("{rate}", &kbps.to_string()));
    }
    if let Some(codecs) = option.codecs.as_ref().filter(|c| !c.is_empty()) {
        parts.push(codecs.clone());
    }
    if let Some(lang) = option.lang.as_ref().filter(|c| !c.is_empty()) {
        parts.push(lang.clone());
    }
    if !option.playable {
        let reason = option
            .sources
            .iter()
            .find_map(|source| source.unsupported_reason.as_deref());
        parts.push(
            match reason {
                Some("ipfs") => ipfs,
                Some("torrent") => torrent,
                _ => cannot_play,
            }
            .to_string(),
        );
    }
    parts.join(" · ")
}

/// Store the choice and, when this episode is already playing, start that version.
pub fn apply_media_selection(episode_id: i32, option_id: String) {
    store_preferred_media_option(episode_id, &option_id);
    let dispatch = Dispatch::<UIState>::global();
    dispatch.reduce_mut(|state| {
        if let Some(plan) = state.episode_page_media.as_mut() {
            if plan.options.iter().any(|option| option.id == option_id) {
                plan.selected_id = option_id.clone();
            }
        }
        if let Some(plan) = state.active_media_plan.as_mut() {
            if plan.options.iter().any(|option| option.id == option_id) {
                plan.selected_id = option_id.clone();
            }
        }
        let playing = state
            .currently_playing
            .as_ref()
            .map(|current| current.episode_id)
            == Some(episode_id);
        if !playing {
            return;
        }
        let Some(plan) = state.active_media_plan.clone() else {
            return;
        };
        if !plan
            .options
            .iter()
            .any(|option| option.id == option_id && option.playable)
        {
            return;
        }
        let steps = reorder_playback(&plan.playback, &option_id);
        let Some(first) = steps.first().cloned() else {
            return;
        };
        state.playback_steps = steps;
        state.playback_step_index = 0;
        state.playback_fell_back = false;
        if let Some(playing) = state.currently_playing.as_mut() {
            playing.src = first.uri.clone();
            playing.is_video = first.is_video;
        }
        let player = dispatch.clone();
        state.set_media_source(first.uri, first.is_video, player);
        if let Some(media) = &state.media_element {
            let _ = media.play();
        }
    });
}

#[derive(Properties, PartialEq)]
pub struct MediaVersionPickerProps {
    pub plan: MediaPlan,
    pub episode_id: i32,
}

#[function_component(MediaVersionPicker)]
pub fn media_version_picker(props: &MediaVersionPickerProps) -> Html {
    if props.plan.options.len() < 2 {
        return html! {};
    }
    let (i18n, _) = use_translation();
    let (ui, _) = use_store::<UIState>();
    let original = i18n.t("media.original").to_string();
    let audio = i18n.t("media.audio").to_string();
    let video = i18n.t("media.video").to_string();
    let hls = i18n.t("media.hls").to_string();
    let dash = i18n.t("media.dash").to_string();
    let other = i18n.t("media.other").to_string();
    let kbps = i18n.t("media.kbps").to_string();
    let ipfs = i18n.t("media.ipfs").to_string();
    let torrent = i18n.t("media.torrent").to_string();
    let cannot_play = i18n.t("media.cannot_play").to_string();
    let choose = i18n.t("media.choose_version").to_string();
    let integrity = i18n.t("media.integrity").to_string();
    let stream_only = i18n.t("media.stream_only").to_string();
    let fell_back = i18n.t("media.fell_back").to_string();

    let selected = if props.plan.options.iter().any(|option| option.id == props.plan.selected_id) {
        props.plan.selected_id.clone()
    } else {
        props.plan.options[0].id.clone()
    };
    let chosen = props
        .plan
        .options
        .iter()
        .find(|option| option.id == selected);
    let show_fallback = ui.playback_fell_back
        && ui
            .currently_playing
            .as_ref()
            .map(|current| current.episode_id)
            == Some(props.episode_id);

    let episode_id = props.episode_id;
    let options = props.plan.options.clone();
    let on_change = Callback::from(move |event: Event| {
        let value = event.target_unchecked_into::<HtmlSelectElement>().value();
        if options.iter().any(|option| option.id == value && option.playable) {
            apply_media_selection(episode_id, value);
        }
    });

    html! {
        <div class="media-version" onclick={Callback::from(|event: MouseEvent| event.stop_propagation())}>
            <label class="media-version-label">{ choose }</label>
            <select value={selected.clone()} onchange={on_change}>
                { for props.plan.options.iter().map(|option| {
                    let label = option_label(
                        option,
                        &original,
                        &audio,
                        &video,
                        &hls,
                        &dash,
                        &other,
                        &kbps,
                        &ipfs,
                        &torrent,
                        &cannot_play,
                    );
                    html! {
                        <option value={option.id.clone()} disabled={!option.playable} selected={option.id == selected}>
                            { label }
                        </option>
                    }
                }) }
            </select>
            { if chosen.and_then(|option| option.integrity.as_ref()).map(|item| item.verifiable).unwrap_or(false) {
                html! { <div class="media-version-note">{ integrity }</div> }
            } else { html! {} } }
            { if chosen.map(|option| option.playable && !option.downloadable).unwrap_or(false) {
                html! { <div class="media-version-note">{ stream_only }</div> }
            } else { html! {} } }
            { if show_fallback {
                html! { <div class="media-version-note">{ fell_back }</div> }
            } else { html! {} } }
        </div>
    }
}
