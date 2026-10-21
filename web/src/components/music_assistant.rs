// Speaker picker, grouping, and the now-playing bar for Music Assistant.
// Rendered only while the per-user bridge is enabled.

use std::collections::HashSet;

use gloo_timers::callback::Interval;
use serde_json::{json, Value};
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlInputElement;
use yew::prelude::*;
use yewdux::prelude::*;

use crate::components::context::{current_playing_anchor, AppState, NotificationState, UIState};
use crate::components::media_options::preferred_media_option;
use crate::requests::music_assistant::{
    choose_playback, explain_test, focus_player, format_clock, is_music_assistant, plan_group,
    player_name, refresh_music_assistant, send_player_command, GroupPlanError,
    MaPlayer, MusicAssistantState, PlaybackCandidate,
};
use crate::requests::now_playing::{send_command, NowPlayingDevice, NowPlayingState};
use crate::requests::notify::{toast_error, toast_info};
use i18nrs::yew::use_translation;

fn session() -> Option<(String, String, i32)> {
    let app = Dispatch::<AppState>::global().get();
    let auth = app.auth_details.as_ref()?;
    let api_key = auth.api_key.clone()?;
    let user_id = app.user_details.as_ref()?.UserID;
    Some((auth.server_name.clone(), api_key, user_id))
}

fn toast_failure(prefix: &str, err: &anyhow::Error) {
    let explained = explain_test("", Err(&err.to_string()));
    let detail = if explained.detail.is_empty() {
        err.to_string()
    } else {
        explained.detail
    };
    toast_error(
        &Dispatch::<NotificationState>::global(),
        format!("{prefix}: {detail}"),
    );
}

fn run_http(
    player_id: String,
    action: &'static str,
    args: Value,
    failure: String,
    pause_local: bool,
) {
    let Some((server, api_key, user_id)) = session() else {
        return;
    };
    spawn_local(async move {
        match send_player_command(&server, &api_key, user_id, &player_id, action, args).await {
            Ok(resp) => {
                if pause_local {
                    let ui = Dispatch::<UIState>::global();
                    if let Some(media) = ui.get().media_element.as_ref() {
                        let _ = media.pause();
                    }
                    ui.reduce_mut(|s| s.audio_playing = Some(false));
                }
                if !resp.warnings.is_empty() {
                    toast_info(
                        &Dispatch::<NotificationState>::global(),
                        resp.warnings.join(" "),
                    );
                }
                refresh_music_assistant();
            }
            Err(err) => toast_failure(&failure, &err),
        }
    });
}

fn now_unix() -> i64 {
    (js_sys::Date::now() / 1000.0) as i64
}

fn set_player_class(active: bool) {
    let Some(body) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.body())
    else {
        return;
    };
    if active {
        let _ = body.class_list().add_1("audio-player-active");
    } else {
        let _ = body.class_list().remove_1("audio-player-active");
    }
}

#[derive(Properties, PartialEq)]
pub(crate) struct VolumeProps {
    pub(crate) device_id: String,
    pub(crate) level: f64,
    pub(crate) muted: bool,
    /// Group volume applies to every member. Speaker volume is just this player.
    pub(crate) group: bool,
    pub(crate) label: String,
}

#[function_component(VolumeControl)]
pub(crate) fn volume_control(props: &VolumeProps) -> Html {
    let (i18n, _) = use_translation();
    let action = if props.group {
        "group_volume"
    } else {
        "volume_set"
    };
    let on_vol = {
        let id = props.device_id.clone();
        Callback::from(move |e: Event| {
            if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                if let Ok(level) = input.value().parse::<f64>() {
                    send_command(&id, action, json!({ "volume_level": level }));
                }
            }
        })
    };
    let on_mute = {
        let id = props.device_id.clone();
        let muted = props.muted;
        Callback::from(move |_: MouseEvent| {
            send_command(&id, "volume_mute", json!({ "muted": !muted }));
        })
    };
    let mute_label = if props.muted {
        i18n.t("music_assistant.unmute").to_string()
    } else {
        i18n.t("music_assistant.mute").to_string()
    };
    html! {
        <div class="ma-volume">
            <span class="ma-volume-label">{ props.label.clone() }</span>
            <input
                type="range"
                min="0"
                max="100"
                step="1"
                value={props.level.to_string()}
                onchange={on_vol}
                aria-label={props.label.clone()}
            />
            <span class="ma-volume-level">{ format!("{:.0}", props.level) }</span>
            <button type="button" class="iconbtn" title={mute_label} onclick={on_mute}>
                <i class={ if props.muted { "ph ph-speaker-slash" } else { "ph ph-speaker-high" } }></i>
            </button>
        </div>
    }
}

fn state_i18n_key(state: &str) -> Option<&'static str> {
    if state.eq_ignore_ascii_case("playing") || state.eq_ignore_ascii_case("buffering") {
        Some("music_assistant.state_playing")
    } else if state.eq_ignore_ascii_case("paused") {
        Some("music_assistant.state_paused")
    } else if state.is_empty()
        || state.eq_ignore_ascii_case("idle")
        || state.eq_ignore_ascii_case("off")
        || state.eq_ignore_ascii_case("standby")
    {
        Some("music_assistant.state_idle")
    } else {
        None
    }
}

/// Fixed bar shown when local playback is idle and an MA player has an episode.
#[function_component(MaNowPlayingBar)]
pub fn ma_now_playing_bar() -> Html {
    let (i18n, _) = use_translation();
    let (ma, ma_dispatch) = use_store::<MusicAssistantState>();
    let (np, _) = use_store::<NowPlayingState>();
    let tick = use_state(|| 0u32);

    let candidates: Vec<PlaybackCandidate> = np
        .devices
        .iter()
        .map(|d| PlaybackCandidate {
            device_id: d.device_id.clone(),
            device_type: d.device_type.clone(),
            episode_id: d.episode_id,
            playing: d.playing,
            updated_at: d.updated_at,
        })
        .collect();
    let device: Option<NowPlayingDevice> = if ma.enabled {
        choose_playback(&candidates, ma.focus_device_id.as_deref()).and_then(|c| {
            np.devices
                .iter()
                .find(|d| d.device_id == c.device_id)
                .cloned()
        })
    } else {
        None
    };

    let playing = device.as_ref().map(|d| d.playing).unwrap_or(false);
    {
        let tick = tick.clone();
        use_effect_with(playing, move |playing| {
            let iv = if *playing {
                let tick = tick.clone();
                Some(Interval::new(1_000, move || {
                    let next = *tick + 1;
                    tick.set(next);
                }))
            } else {
                None
            };
            move || drop(iv)
        });
    }
    {
        let active = device.is_some();
        use_effect_with(active, move |active| {
            set_player_class(*active);
            move || set_player_class(false)
        });
    }
    let _ = *tick;

    let Some(device) = device else {
        return html! {};
    };

    let shown = crate::requests::music_assistant::displayed_position(
        device.position_sec,
        device.playing,
        device.speed,
        device.updated_at,
        now_unix(),
    );
    let duration = if device.duration_sec.is_finite() {
        device.duration_sec.max(0.0)
    } else {
        0.0
    };
    let progress = if duration > 0.0 {
        format!(
            "{} / {}",
            format_clock(shown.min(duration)),
            format_clock(duration)
        )
    } else {
        format_clock(shown)
    };
    let title = if device.title.is_empty() {
        i18n.t("music_assistant.now_playing").to_string()
    } else {
        device.title.clone()
    };
    let id = device.device_id.clone();
    let is_playing = device.playing;
    let on_play_pause = {
        let id = id.clone();
        Callback::from(move |_: MouseEvent| {
            send_command(
                &id,
                if is_playing { "pause" } else { "play" },
                json!({}),
            );
        })
    };
    let on_stop = {
        let id = id.clone();
        Callback::from(move |_: MouseEvent| {
            send_command(&id, "stop", json!({}));
        })
    };
    let on_back = {
        let id = id.clone();
        Callback::from(move |_: MouseEvent| {
            send_command(&id, "skip_back", json!({ "seconds": 15 }));
        })
    };
    let on_fwd = {
        let id = id.clone();
        Callback::from(move |_: MouseEvent| {
            send_command(&id, "skip_forward", json!({ "seconds": 30 }));
        })
    };
    let on_seek = {
        let id = id.clone();
        Callback::from(move |e: Event| {
            if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                if let Ok(pos) = input.value().parse::<f64>() {
                    send_command(&id, "seek", json!({ "position_sec": pos }));
                }
            }
        })
    };
    let on_speed = {
        let id = id.clone();
        Callback::from(move |e: Event| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlSelectElement>() {
                if let Ok(speed) = input.value().parse::<f64>() {
                    send_command(&id, "set_speed", json!({ "speed": speed }));
                }
            }
        })
    };
    let open_picker = {
        let ma_dispatch = ma_dispatch.clone();
        Callback::from(move |_: MouseEvent| {
            ma_dispatch.reduce_mut(|s| {
                s.pending_episode = None;
                s.picker_open = true;
            });
        })
    };
    let speed = if device.speed > 0.0 { device.speed } else { 1.0 };
    let speeds = [0.5_f64, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0];
    let members = device.group_members.clone().unwrap_or_default();
    let volume = device.volume_level.unwrap_or(0.0);
    let muted = device.volume_muted.unwrap_or(false);
    let volume_label = if members.is_empty() {
        i18n.t("music_assistant.volume").to_string()
    } else {
        i18n.t("music_assistant.group_volume").to_string()
    };

    html! {
        <div class="audio-player ma-now">
            <div class="ma-now-top">
                if !device.artwork_url.is_empty() {
                    <img class="ma-art" src={device.artwork_url.clone()} alt="" />
                }
                <div class="ma-now-meta">
                    <div class="ma-now-kicker">
                        { i18n.t("music_assistant.playing_on_ma").to_string().replace("{name}", &device.device_name) }
                    </div>
                    <div class="ma-now-title">{ title }</div>
                    <div class="ma-now-progress">{ progress }</div>
                </div>
                <div class="ma-now-transport">
                    if device.supports("skip_back") {
                        <button type="button" class="iconbtn" title={i18n.t("music_assistant.skip_back").to_string()} onclick={on_back}>
                            <i class="ph ph-skip-back"></i>
                        </button>
                    }
                    if device.supports(if is_playing { "pause" } else { "play" }) {
                        <button type="button" class="iconbtn" title={ if is_playing { i18n.t("music_assistant.pause").to_string() } else { i18n.t("music_assistant.play").to_string() } } onclick={on_play_pause}>
                            <i class={ if is_playing { "ph ph-pause" } else { "ph ph-play" } }></i>
                        </button>
                    }
                    if device.supports("skip_forward") {
                        <button type="button" class="iconbtn" title={i18n.t("music_assistant.skip_forward").to_string()} onclick={on_fwd}>
                            <i class="ph ph-skip-forward"></i>
                        </button>
                    }
                    if device.supports("stop") {
                        <button type="button" class="iconbtn" title={i18n.t("music_assistant.stop").to_string()} onclick={on_stop}>
                            <i class="ph ph-stop"></i>
                        </button>
                    }
                    if device.supports("set_speed") {
                        <select class="select ma-speed" onchange={on_speed} aria-label={i18n.t("music_assistant.speed").to_string()}>
                            { for speeds.iter().map(|rate| html! {
                                <option value={rate.to_string()} selected={(speed - rate).abs() < 0.05}>{ format!("{rate:.2}x") }</option>
                            }) }
                        </select>
                    }
                    <button type="button" class="rd-play-here" onclick={open_picker}>
                        <i class="ph ph-speaker-high mr-1"></i>{ i18n.t("music_assistant.speakers") }
                    </button>
                </div>
            </div>
            if duration > 0.0 && device.supports("seek") {
                <input
                    class="ma-seek"
                    type="range"
                    min="0"
                    max={duration.to_string()}
                    step="1"
                    value={shown.min(duration).to_string()}
                    onchange={on_seek}
                    aria-label={i18n.t("music_assistant.seek").to_string()}
                />
            }
            if device.supports(if members.is_empty() { "volume_set" } else { "group_volume" }) {
                <VolumeControl
                    device_id={id.clone()}
                    level={volume}
                    muted={muted}
                    group={!members.is_empty()}
                    label={volume_label}
                />
            }
            if !members.is_empty() {
                <div class="ma-members">
                    <div class="ma-volume-label">{ i18n.t("music_assistant.members") }</div>
                    { for members.iter().filter_map(|member| {
                        let player = ma.players.iter().find(|p| p.player_id == *member)?;
                        if !player.available {
                            return None;
                        }
                        let label = if player.name.is_empty() { member.clone() } else { player.name.clone() };
                        Some(html! {
                            <VolumeControl
                                key={player.player_id.clone()}
                                device_id={player.device_id.clone()}
                                level={player.volume_level.unwrap_or(0.0)}
                                muted={player.volume_muted}
                                group={false}
                                label={label}
                            />
                        })
                    }) }
                </div>
            }
        </div>
    }
}

#[function_component(MusicAssistantHost)]
pub fn music_assistant_host() -> Html {
    let server = use_selector(|s: &AppState| {
        s.auth_details.as_ref().map(|a| a.server_name.clone())
    });
    let api_key = use_selector(|s: &AppState| {
        s.auth_details.as_ref().and_then(|a| a.api_key.clone())
    });
    let user_id = use_selector(|s: &AppState| s.user_details.as_ref().map(|u| u.UserID));
    {
        let deps = ((*server).clone(), (*api_key).clone(), *user_id);
        use_effect_with(deps, move |_| {
            refresh_music_assistant();
            let iv = Interval::new(12_000, refresh_music_assistant);
            move || drop(iv)
        });
    }
    let (ma, _) = use_store::<MusicAssistantState>();
    html! {
        if ma.enabled {
            <SpeakerPicker />
        }
    }
}

#[function_component(SpeakerPicker)]
fn speaker_picker() -> Html {
    let (i18n, _) = use_translation();
    let (ma, ma_dispatch) = use_store::<MusicAssistantState>();
    let (ui, _) = use_store::<UIState>();
    let selected = use_state(HashSet::<String>::new);
    let open = ma.picker_open;

    let close = {
        let ma_dispatch = ma_dispatch.clone();
        Callback::from(move |_: MouseEvent| {
            ma_dispatch.reduce_mut(|s| {
                s.picker_open = false;
                s.pending_episode = None;
            });
        })
    };
    let stop = Callback::from(|e: MouseEvent| e.stop_propagation());

    let failure = i18n.t("music_assistant.command_failed").to_string();
    let nothing = i18n.t("music_assistant.nothing_to_play").to_string();
    let need_two = i18n.t("music_assistant.group_need_two").to_string();
    let multi = i18n.t("music_assistant.group_one_existing").to_string();
    let unchanged = i18n.t("music_assistant.group_unchanged").to_string();

    let play_target = |pending: &Option<crate::requests::music_assistant::PendingEpisode>,
                       ui: &UIState|
     -> Option<(i32, bool, Option<f64>)> {
        if let Some(pending) = pending {
            return Some((pending.episode_id, pending.is_youtube, None));
        }
        let (episode_id, is_youtube) = current_playing_anchor();
        let episode_id = episode_id?;
        let position = ui
            .media_element
            .as_ref()
            .map(|m| m.current_time())
            .unwrap_or(ui.current_time_seconds);
        Some((episode_id, is_youtube.unwrap_or(false), Some(position)))
    };

    let on_play = {
        let failure = failure.clone();
        let nothing = nothing.clone();
        let pending = ma.pending_episode.clone();
        let ui = ui.clone();
        let ma_dispatch = ma_dispatch.clone();
        Callback::from(move |player: MaPlayer| {
            let Some((episode_id, is_youtube, position)) = play_target(&pending, &ui) else {
                toast_info(&Dispatch::<NotificationState>::global(), nothing.clone());
                return;
            };
            let mut args = json!({
                "episode_id": episode_id,
                "is_youtube": is_youtube,
            });
            if let Some(position) = position {
                args["position_sec"] = json!(position);
            }
            if !is_youtube {
                if let Some(option_id) = preferred_media_option(episode_id) {
                    args["media_option_id"] = json!(option_id);
                }
            }
            focus_player(&player.device_id);
            ma_dispatch.reduce_mut(|s| {
                s.picker_open = false;
                s.pending_episode = None;
            });
            run_http(player.player_id, "play_episode", args, failure.clone(), true);
        })
    };

    let on_group = {
        let selected = selected.clone();
        let players = ma.players.clone();
        let failure = failure.clone();
        let need_two = need_two.clone();
        let multi = multi.clone();
        let unchanged = unchanged.clone();
        Callback::from(move |_: MouseEvent| {
            let ids: Vec<String> = players
                .iter()
                .filter(|p| selected.contains(&p.player_id))
                .map(|p| p.player_id.clone())
                .collect();
            match plan_group(&players, &ids) {
                Ok(plan) => {
                    let player_id = plan.player_id().to_string();
                    let action = plan.action();
                    let args = plan.args();
                    selected.set(HashSet::new());
                    run_http(player_id, action, args, failure.clone(), false);
                }
                Err(GroupPlanError::NeedTwo) => {
                    toast_info(&Dispatch::<NotificationState>::global(), need_two.clone())
                }
                Err(GroupPlanError::MultipleGroups) => {
                    toast_info(&Dispatch::<NotificationState>::global(), multi.clone())
                }
                Err(GroupPlanError::Unchanged) => {
                    toast_info(&Dispatch::<NotificationState>::global(), unchanged.clone())
                }
            }
        })
    };

    let checked_count = ma
        .players
        .iter()
        .filter(|p| selected.contains(&p.player_id))
        .count();
    let play_label = if ma.pending_episode.is_some() {
        i18n.t("music_assistant.play_episode").to_string()
    } else {
        i18n.t("music_assistant.play_current").to_string()
    };

    html! {
        <>
            <div class={classes!("nc-scrim", open.then_some("is-open"))} onclick={close.clone()}></div>
            <div class={classes!("nc-panel", "nc-form-drawer", "ma-picker", open.then_some("is-open"))}
                 data-density="compact" onclick={stop}>
                <div class="nc-header">
                    <h3 class="nc-h-title">{ i18n.t("music_assistant.speaker_picker") }</h3>
                    <div class="nc-h-spacer"></div>
                    <button type="button" class="nc-h-btn" title={i18n.t("music_assistant.close").to_string()} onclick={close}>
                        <i class="ph ph-arrow-line-right"></i>
                    </button>
                </div>
                <div class="nc-body">
                    if !ma.connected {
                        <div class="ma-note">{ i18n.t("music_assistant.not_connected") }</div>
                    }
                    if ma.players.is_empty() {
                        <div class="nc-empty">
                            <span class="nc-empty-ico"><i class="ph ph-speaker-high"></i></span>
                            <div class="nc-empty-t">{ i18n.t("music_assistant.no_players") }</div>
                        </div>
                    }
                    <div class="ma-note">{ i18n.t("music_assistant.select_speakers") }</div>
                    { for ma.players.iter().map(|player| {
                        let id = player.player_id.clone();
                        let checked = selected.contains(&id);
                        let on_check = {
                            let selected = selected.clone();
                            let id = id.clone();
                            Callback::from(move |_: Event| {
                                let mut next = (*selected).clone();
                                if !next.insert(id.clone()) {
                                    next.remove(&id);
                                }
                                selected.set(next);
                            })
                        };
                        let play = {
                            let on_play = on_play.clone();
                            let player = player.clone();
                            Callback::from(move |_: MouseEvent| on_play.emit(player.clone()))
                        };
                        let kind = if player.kind.eq_ignore_ascii_case("group") || !player.group_members.is_empty() {
                            i18n.t("music_assistant.group_kind").to_string()
                        } else {
                            i18n.t("music_assistant.player").to_string()
                        };
                        let state = match state_i18n_key(&player.state) {
                            Some(key) => i18n.t(key).to_string(),
                            None => player.state.clone(),
                        };
                        let unavailable = if player.available {
                            String::new()
                        } else {
                            i18n.t("music_assistant.unavailable").to_string()
                        };
                        let synced = player.synced_to.as_ref().map(|id| {
                            player_name(&ma.players, id).to_string()
                        });
                        let ungroup = {
                            let failure = failure.clone();
                            let id = id.clone();
                            Callback::from(move |_: MouseEvent| {
                                run_http(id.clone(), "ungroup", json!({}), failure.clone(), false);
                            })
                        };
                        let remove_member = {
                            let failure = failure.clone();
                            let group_id = id.clone();
                            Callback::from(move |member: String| {
                                run_http(
                                    group_id.clone(),
                                    "set_members",
                                    json!({ "player_ids_to_remove": [member] }),
                                    failure.clone(),
                                    false,
                                );
                            })
                        };
                        let add_member = {
                            let failure = failure.clone();
                            let group_id = id.clone();
                            Callback::from(move |e: Event| {
                                let Some(input) = e.target_dyn_into::<web_sys::HtmlSelectElement>() else {
                                    return;
                                };
                                let member = input.value();
                                if member.is_empty() {
                                    return;
                                }
                                run_http(
                                    group_id.clone(),
                                    "set_members",
                                    json!({ "player_ids_to_add": [member] }),
                                    failure.clone(),
                                    false,
                                );
                            })
                        };
                        let show_group_tools = player.is_group();
                        html! {
                            <div class="rd-row" key={player.player_id.clone()}>
                                <div class="rd-row-top">
                                    <label class="ma-check">
                                        <input type="checkbox" checked={checked} onchange={on_check} />
                                        <span class="rd-name">
                                            <i class={ if show_group_tools { "ph ph-speakers" } else { "ph ph-speaker-high" } }></i>
                                            { player.name.clone() }
                                        </span>
                                    </label>
                                    <span class={classes!("rd-state", (player.state == "playing").then_some("is-playing"))}>
                                        { kind }{ " · " }{ state }
                                        if !unavailable.is_empty() {
                                            { format!(" · {unavailable}") }
                                        }
                                    </span>
                                </div>
                                if let Some(leader) = synced {
                                    <span class="rd-track">{ format!("{} {leader}", i18n.t("music_assistant.synced_to")) }</span>
                                }
                                if show_group_tools {
                                    <div class="ma-member-list">
                                        { for player.group_members.iter().map(|member| {
                                            let label = player_name(&ma.players, member).to_string();
                                            let remove = {
                                                let remove_member = remove_member.clone();
                                                let member = member.clone();
                                                Callback::from(move |_: MouseEvent| remove_member.emit(member.clone()))
                                            };
                                            html! {
                                                <span class="ma-chip" key={member.clone()}>
                                                    { label }
                                                    <button type="button" class="iconbtn" title={i18n.t("music_assistant.remove_member").to_string()} onclick={remove}>
                                                        <i class="ph ph-x"></i>
                                                    </button>
                                                </span>
                                            }
                                        }) }
                                    </div>
                                    if !player.can_group_with.is_empty() {
                                        <select class="select ma-add-member" onchange={add_member}>
                                            <option value="">{ i18n.t("music_assistant.add_member") }</option>
                                            { for player.can_group_with.iter().filter(|id| !player.group_members.iter().any(|m| m == *id)).map(|id| {
                                                let label = player_name(&ma.players, id).to_string();
                                                html! { <option value={id.clone()}>{ label }</option> }
                                            }) }
                                        </select>
                                    }
                                }
                                if player.available {
                                    if let Some(level) = player.volume_level {
                                        if show_group_tools {
                                            <VolumeControl
                                                device_id={player.device_id.clone()}
                                                level={level}
                                                muted={player.volume_muted}
                                                group={true}
                                                label={i18n.t("music_assistant.group_volume").to_string()}
                                            />
                                        } else {
                                            <VolumeControl
                                                device_id={player.device_id.clone()}
                                                level={level}
                                                muted={player.volume_muted}
                                                group={false}
                                                label={i18n.t("music_assistant.volume").to_string()}
                                            />
                                        }
                                    }
                                }
                                <div class="rd-controls">
                                    <button type="button" class="rd-play-here" disabled={!player.available} onclick={play}>
                                        <i class="ph ph-play mr-1"></i>{ play_label.clone() }
                                    </button>
                                    if show_group_tools {
                                        <button type="button" class="rd-play-here" onclick={ungroup}>
                                            { i18n.t("music_assistant.ungroup") }
                                        </button>
                                    }
                                </div>
                            </div>
                        }
                    }) }
                    <div class="ma-group-bar">
                        <button type="button" class="btn btn-primary" onclick={on_group} disabled={checked_count < 2}>
                            { i18n.t("music_assistant.group") }
                        </button>
                    </div>
                </div>
            </div>
        </>
    }
}

/// True when a relay device is a Music Assistant player. Used by the remote panel.
pub fn device_is_ma(device_type: &str) -> bool {
    is_music_assistant(device_type)
}
