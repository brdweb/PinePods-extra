//! Value for Value wallet settings. Payments stay off until this is enabled
//! and a daily limit above zero is saved.

use crate::components::context::AppState;
use crate::requests::value4value::{get_settings, save_settings, test_wallet};
use i18nrs::yew::use_translation;
use web_sys::HtmlInputElement;
use yew::prelude::*;
use yewdux::prelude::*;

fn input_value(e: InputEvent) -> String {
    e.target_dyn_into::<HtmlInputElement>()
        .map(|input| input.value())
        .unwrap_or_default()
}

fn parse_sats(raw: &str) -> i64 {
    raw.trim().parse::<i64>().unwrap_or(0).max(0)
}

#[function_component(ValueSettings)]
pub fn value_settings() -> Html {
    let (i18n, _) = use_translation();
    let (state, _) = use_store::<AppState>();
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let api_key = state.auth_details.as_ref().and_then(|ud| ud.api_key.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID);

    let enabled = use_state(|| false);
    let secret = use_state(String::new);
    let has_secret = use_state(|| false);
    let clear_secret = use_state(|| false);
    let max_boost = use_state(|| "1000".to_string());
    let stream_rate = use_state(|| "0".to_string());
    let daily_limit = use_state(|| "0".to_string());
    let spent = use_state(|| 0i64);
    let saving = use_state(|| false);
    let testing = use_state(|| false);
    let notice = use_state(|| Option::<(bool, String)>::None);

    {
        let enabled = enabled.clone();
        let has_secret = has_secret.clone();
        let max_boost = max_boost.clone();
        let stream_rate = stream_rate.clone();
        let daily_limit = daily_limit.clone();
        let spent = spent.clone();
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let user_id = user_id;
        use_effect_with((), move |_| {
            if let (Some(server_name), Some(api_key), Some(user_id)) = (server_name, api_key, user_id) {
                wasm_bindgen_futures::spawn_local(async move {
                    if let Ok(view) = get_settings(&server_name, &api_key, user_id).await {
                        enabled.set(view.enabled);
                        has_secret.set(view.has_secret);
                        max_boost.set(view.max_boost_sats.to_string());
                        stream_rate.set(view.stream_sats_per_minute.to_string());
                        daily_limit.set(view.daily_limit_sats.to_string());
                        spent.set(view.spent_today_sats);
                    }
                });
            }
            || ()
        });
    }

    let on_enabled = {
        let enabled = enabled.clone();
        Callback::from(move |_: Event| enabled.set(!*enabled))
    };
    let on_secret = {
        let secret = secret.clone();
        Callback::from(move |e: InputEvent| secret.set(input_value(e)))
    };
    let on_clear = {
        let clear_secret = clear_secret.clone();
        Callback::from(move |_: Event| clear_secret.set(!*clear_secret))
    };
    let on_max = {
        let max_boost = max_boost.clone();
        Callback::from(move |e: InputEvent| max_boost.set(input_value(e)))
    };
    let on_stream = {
        let stream_rate = stream_rate.clone();
        Callback::from(move |e: InputEvent| stream_rate.set(input_value(e)))
    };
    let on_daily = {
        let daily_limit = daily_limit.clone();
        Callback::from(move |e: InputEvent| daily_limit.set(input_value(e)))
    };

    let on_save = {
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let user_id = user_id;
        let enabled = enabled.clone();
        let secret = secret.clone();
        let clear_secret = clear_secret.clone();
        let has_secret = has_secret.clone();
        let max_boost = max_boost.clone();
        let stream_rate = stream_rate.clone();
        let daily_limit = daily_limit.clone();
        let spent = spent.clone();
        let saving = saving.clone();
        let notice = notice.clone();
        let saved_label = i18n.t("value4value.saved").to_string();
        Callback::from(move |_| {
            let (Some(server_name), Some(api_key), Some(user_id)) = (server_name.clone(), api_key.clone(), user_id) else {
                return;
            };
            let enabled_val = *enabled;
            let secret_val = (*secret).clone();
            let clear_val = *clear_secret;
            let max_val = parse_sats(&*max_boost);
            let stream_val = parse_sats(&*stream_rate);
            let daily_val = parse_sats(&*daily_limit);
            let saving = saving.clone();
            let notice = notice.clone();
            let has_secret = has_secret.clone();
            let secret = secret.clone();
            let clear_secret = clear_secret.clone();
            let spent = spent.clone();
            let saved_label = saved_label.clone();
            saving.set(true);
            wasm_bindgen_futures::spawn_local(async move {
                match save_settings(
                    &server_name,
                    &api_key,
                    user_id,
                    enabled_val,
                    &secret_val,
                    clear_val,
                    max_val,
                    stream_val,
                    daily_val,
                )
                .await
                {
                    Ok(view) => {
                        has_secret.set(view.has_secret);
                        secret.set(String::new());
                        clear_secret.set(false);
                        spent.set(view.spent_today_sats);
                        notice.set(Some((true, saved_label)));
                    }
                    Err(err) => notice.set(Some((false, err))),
                }
                saving.set(false);
            });
        })
    };

    let on_test = {
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let user_id = user_id;
        let secret = secret.clone();
        let testing = testing.clone();
        let notice = notice.clone();
        let balance_template = i18n.t("value4value.balance").to_string();
        Callback::from(move |_| {
            let (Some(server_name), Some(api_key), Some(user_id)) = (server_name.clone(), api_key.clone(), user_id) else {
                return;
            };
            let secret_val = (*secret).clone();
            let testing = testing.clone();
            let notice = notice.clone();
            let balance_template = balance_template.clone();
            testing.set(true);
            wasm_bindgen_futures::spawn_local(async move {
                match test_wallet(&server_name, &api_key, user_id, &secret_val).await {
                    Ok(report) if report.ok => {
                        let text = balance_template.replace("{sats}", &report.balance_sats.to_string());
                        notice.set(Some((true, text)));
                    }
                    Ok(report) => notice.set(Some((false, report.detail))),
                    Err(err) => notice.set(Some((false, err))),
                }
                testing.set(false);
            });
        })
    };

    let spent_line = i18n
        .t("value4value.spent")
        .replace("{sats}", &spent.to_string());
    let secret_placeholder = if *has_secret {
        i18n.t("value4value.secret_stored").to_string()
    } else {
        String::new()
    };

    html! {
        <>
            <div class="settings-row">
                <div class="settings-row-desc">{ i18n.t("value4value.banner") }</div>
            </div>
            <div class="settings-row">
                <div>
                    <div class="settings-row-label">{ i18n.t("value4value.enabled") }</div>
                    <div class="settings-row-desc">{ i18n.t("value4value.enabled_desc") }</div>
                </div>
                <div class="settings-row-control">
                    <label class="toggle">
                        <input type="checkbox" checked={*enabled} onchange={on_enabled} />
                        <span class="toggle-track"><span class="toggle-thumb"></span></span>
                    </label>
                </div>
            </div>
            <div class="settings-row">
                <div>
                    <div class="settings-row-label">{ i18n.t("value4value.secret") }</div>
                    <div class="settings-row-desc">{ i18n.t("value4value.secret_desc") }</div>
                    <input
                        type="password"
                        class="form-input w-full"
                        autocomplete="off"
                        placeholder={secret_placeholder}
                        value={(*secret).clone()}
                        oninput={on_secret}
                    />
                    if *has_secret {
                        <label class="ma-check">
                            <input type="checkbox" checked={*clear_secret} onchange={on_clear} />
                            <span>{ i18n.t("value4value.clear_secret") }</span>
                        </label>
                    }
                </div>
            </div>
            <div class="settings-row">
                <div>
                    <div class="settings-row-label">{ i18n.t("value4value.max_boost") }</div>
                    <input type="number" min="0" class="form-input" value={(*max_boost).clone()} oninput={on_max} />
                </div>
            </div>
            <div class="settings-row">
                <div>
                    <div class="settings-row-label">{ i18n.t("value4value.stream") }</div>
                    <div class="settings-row-desc">{ i18n.t("value4value.stream_desc") }</div>
                    <input type="number" min="0" class="form-input" value={(*stream_rate).clone()} oninput={on_stream} />
                </div>
            </div>
            <div class="settings-row">
                <div>
                    <div class="settings-row-label">{ i18n.t("value4value.daily") }</div>
                    <div class="settings-row-desc">{ i18n.t("value4value.daily_desc") }</div>
                    <input type="number" min="0" class="form-input" value={(*daily_limit).clone()} oninput={on_daily} />
                    <div class="settings-row-desc">{ spent_line }</div>
                </div>
            </div>
            if let Some((ok, text)) = &*notice {
                <div class="settings-row">
                    <div class={classes!("ma-note", if *ok { "is-ok" } else { "is-error" })}>{ text.clone() }</div>
                </div>
            }
            <div class="settings-row">
                <div></div>
                <div class="settings-row-control" style="gap:8px;">
                    <button type="button" class="btn btn-secondary" onclick={on_test} disabled={*testing}>
                        <span>{ if *testing { i18n.t("value4value.testing") } else { i18n.t("value4value.test") } }</span>
                    </button>
                    <button type="button" class="btn btn-primary" onclick={on_save} disabled={*saving}>
                        <span>{ if *saving { i18n.t("value4value.saving") } else { i18n.t("value4value.save") } }</span>
                    </button>
                </div>
            </div>
        </>
    }
}
