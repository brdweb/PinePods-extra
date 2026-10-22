//! Boost form on an episode. Hidden unless the stored value block can be paid.
//! The button stays disabled until the listener confirms the sats are real.

use crate::components::context::{AppState, UIState};
use crate::requests::login_requests::GetUserDetails;
use crate::requests::value4value::{preview_episode, send_boost, ValuePreview};
use i18nrs::yew::use_translation;
use web_sys::HtmlInputElement;
use yew::prelude::*;
use yewdux::prelude::*;

fn input_value(e: InputEvent) -> String {
    e.target_dyn_into::<HtmlInputElement>()
        .map(|input| input.value())
        .unwrap_or_default()
}

fn sender_name(user: Option<&GetUserDetails>) -> String {
    user.and_then(|user| {
        user.Fullname
            .clone()
            .filter(|name| !name.trim().is_empty())
            .or_else(|| user.Username.clone())
    })
    .unwrap_or_default()
}

#[derive(Properties, PartialEq, Clone)]
pub struct ValueBoostProps {
    pub episode_id: i32,
    pub api_key: String,
    pub user_id: i32,
    pub server_name: String,
}

#[function_component(ValueBoost)]
pub fn value_boost(props: &ValueBoostProps) -> Html {
    let (i18n, _) = use_translation();
    let (state, _) = use_store::<AppState>();
    let (ui, _) = use_store::<UIState>();
    let preview = use_state(|| Option::<ValuePreview>::None);
    let sats = use_state(String::new);
    let message = use_state(String::new);
    let confirmed = use_state(|| false);
    let sending = use_state(|| false);
    let result = use_state(|| Option::<(bool, String)>::None);
    let filled = use_state(|| false);

    {
        let preview = preview.clone();
        let sats = sats.clone();
        let filled = filled.clone();
        let server = props.server_name.clone();
        let api_key = props.api_key.clone();
        let user_id = props.user_id;
        let episode_id = props.episode_id;
        use_effect_with(episode_id, move |_| {
            if episode_id > 0 && !server.is_empty() && !api_key.is_empty() && user_id > 0 {
                wasm_bindgen_futures::spawn_local(async move {
                    if let Ok(view) = preview_episode(&server, &api_key, user_id, episode_id).await {
                        if !*filled {
                            if let Some(suggested) = view.suggested_sats {
                                sats.set(suggested.to_string());
                                filled.set(true);
                            }
                        }
                        preview.set(Some(view));
                    }
                });
            }
            || ()
        });
    }

    let Some(view) = &*preview else {
        return html! {};
    };
    if !view.payable {
        return html! {};
    }

    let on_sats = {
        let sats = sats.clone();
        Callback::from(move |e: InputEvent| sats.set(input_value(e)))
    };
    let on_message = {
        let message = message.clone();
        Callback::from(move |e: InputEvent| message.set(input_value(e)))
    };
    let on_confirm = {
        let confirmed = confirmed.clone();
        Callback::from(move |_: Event| confirmed.set(!*confirmed))
    };

    let amount: i64 = sats.trim().parse().unwrap_or(0);
    let can_send = *confirmed && amount > 0 && !*sending;
    let position = if ui.currently_playing.as_ref().map(|item| item.episode_id) == Some(props.episode_id) {
        ui.current_time_seconds as i64
    } else {
        0
    };
    let name = sender_name(state.user_details.as_ref());

    let on_send = {
        let server = props.server_name.clone();
        let api_key = props.api_key.clone();
        let user_id = props.user_id;
        let episode_id = props.episode_id;
        let message = message.clone();
        let sending = sending.clone();
        let result = result.clone();
        let confirmed = confirmed.clone();
        let sent_template = i18n.t("value4value.sent").to_string();
        let failed_label = i18n.t("value4value.failed").to_string();
        Callback::from(move |_| {
            if !can_send {
                return;
            }
            let server = server.clone();
            let api_key = api_key.clone();
            let message_val = (*message).clone();
            let name = name.clone();
            let sending = sending.clone();
            let result = result.clone();
            let confirmed = confirmed.clone();
            let sent_template = sent_template.clone();
            let failed_label = failed_label.clone();
            sending.set(true);
            wasm_bindgen_futures::spawn_local(async move {
                match send_boost(
                    &server,
                    &api_key,
                    user_id,
                    episode_id,
                    amount,
                    &message_val,
                    &name,
                    position,
                )
                .await
                {
                    Ok(report) if report.sent => {
                        let text = sent_template.replace("{sats}", &report.sats_sent.to_string());
                        result.set(Some((true, text)));
                        confirmed.set(false);
                    }
                    Ok(report) => {
                        let text = if report.reason.is_empty() { failed_label } else { report.reason };
                        result.set(Some((false, text)));
                    }
                    Err(err) => result.set(Some((false, err))),
                }
                sending.set(false);
            });
        })
    };

    let suggested = view.suggested_sats.map(|sats| {
        i18n.t("value4value.suggested").replace("{sats}", &sats.to_string())
    });

    html! {
        <section class="p2-block">
            <div class="p2-head">
                <span class="p2-title">{ i18n.t("value4value.boost") }</span>
            </div>
            <p class="p2-row-meta">{ i18n.t("value4value.real") }</p>
            if let Some(suggested) = suggested {
                <p class="p2-row-meta">{ suggested }</p>
            }
            <input type="number" min="1" class="form-input" value={(*sats).clone()} oninput={on_sats} />
            <input
                type="text"
                class="form-input"
                placeholder={i18n.t("value4value.message").to_string()}
                value={(*message).clone()}
                oninput={on_message}
            />
            <label class="ma-check">
                <input type="checkbox" checked={*confirmed} onchange={on_confirm} />
                <span>{ i18n.t("value4value.confirm") }</span>
            </label>
            <button type="button" class="btn btn-primary" onclick={on_send} disabled={!can_send}>
                <span>{ if *sending { i18n.t("value4value.sending") } else { i18n.t("value4value.send") } }</span>
            </button>
            if let Some((ok, text)) = &*result {
                <p class={classes!("p2-row-meta", if *ok { "is-ok" } else { "is-error" })}>{ text.clone() }</p>
            }
        </section>
    }
}
