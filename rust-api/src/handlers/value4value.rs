//! Value4Value settings and payments.
//!
//! The wallet connection string is write-only. Boost and stream payments load
//! recipients from the stored feed, so a client cannot redirect sats.

use axum::{extract::Query, extract::State, http::HeaderMap, Json};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::handlers::{extract_api_key, require_user_access, require_valid_api_key};
use crate::services::value4value::{self, SendReport, SettingsUpdate};
use crate::AppState;

#[derive(Deserialize, utoipa::IntoParams)]
pub struct ValueUserQuery {
    pub user_id: i32,
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct ValueEpisodeQuery {
    pub user_id: i32,
    pub episode_id: i32,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct ValueSettingsResponse {
    pub enabled: bool,
    pub has_secret: bool,
    pub max_boost_sats: i64,
    pub stream_sats_per_minute: i64,
    pub daily_limit_sats: i64,
    pub spent_today_sats: i64,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct ValueSettingsBody {
    pub user_id: i32,
    /// Nothing is sent until this is true and the daily limit is above 0.
    #[serde(default)]
    pub enabled: bool,
    /// Nostr Wallet Connect string. Omit or send an empty string to keep the stored one.
    pub secret: Option<String>,
    #[serde(default)]
    pub clear_secret: bool,
    pub max_boost_sats: i64,
    pub stream_sats_per_minute: i64,
    pub daily_limit_sats: i64,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct ValueTestBody {
    pub user_id: i32,
    /// Tried for this probe only. Never written.
    pub secret: Option<String>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct ValueTestResponse {
    pub ok: bool,
    pub balance_sats: u64,
    pub detail: String,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct ValuePreviewResponse {
    pub payable: bool,
    pub suggested_sats: Option<i64>,
    pub recipient_count: i64,
    /// A time split named a remote item without inlined recipients. That feed is not loaded.
    pub remote_feed_not_loaded: bool,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct ValueBoostBody {
    pub user_id: i32,
    pub episode_id: i32,
    pub sats: i64,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub sender_name: String,
    /// Playhead in seconds, used for valueTimeSplit and the boostagram.
    #[serde(default)]
    pub position_seconds: i64,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct ValueStreamBody {
    pub user_id: i32,
    pub episode_id: i32,
    pub position_seconds: i64,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct ValueParty {
    pub name: String,
    pub sats: i64,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub detail: String,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct ValueSendResponse {
    pub sent: bool,
    pub reason: String,
    pub sats_sent: i64,
    pub paid: Vec<ValueParty>,
    pub failed: Vec<ValueParty>,
    pub skipped: Vec<ValueParty>,
    pub unallocated_sats: i64,
    pub remote_feed_not_loaded: bool,
}

async fn authorize(state: &AppState, headers: &HeaderMap, user_id: i32) -> Result<(), AppError> {
    let api_key = extract_api_key(headers)?;
    require_valid_api_key(state, &api_key).await?;
    require_user_access(state, &api_key, user_id).await?;
    Ok(())
}

fn settings_response(view: value4value::WalletSettings) -> ValueSettingsResponse {
    ValueSettingsResponse {
        enabled: view.enabled,
        has_secret: view.has_secret,
        max_boost_sats: view.max_boost_sats,
        stream_sats_per_minute: view.stream_sats_per_minute,
        daily_limit_sats: view.daily_limit_sats,
        spent_today_sats: view.spent_today_sats,
    }
}

fn send_response(report: SendReport) -> ValueSendResponse {
    ValueSendResponse {
        sent: report.sent,
        reason: report.reason,
        sats_sent: report.sats_sent as i64,
        paid: report
            .paid
            .into_iter()
            .map(|item| ValueParty {
                name: item.name,
                sats: item.sats as i64,
                detail: String::new(),
            })
            .collect(),
        failed: report
            .failed
            .into_iter()
            .map(|item| ValueParty {
                name: item.name,
                sats: item.sats as i64,
                detail: item.error,
            })
            .collect(),
        skipped: report
            .skipped
            .into_iter()
            .map(|item| ValueParty {
                name: item.name,
                sats: 0,
                detail: format!("{} ({})", item.reason, item.kind),
            })
            .collect(),
        unallocated_sats: report.unallocated_sats as i64,
        remote_feed_not_loaded: report.remote_feed_not_loaded,
    }
}

#[utoipa::path(
    get,
    path = "/value/settings",
    tag = "value4value",
    summary = "Get Value4Value settings",
    params(ValueUserQuery),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Per-user wallet settings. The connection string is never returned.", body = ValueSettingsResponse),
        (status = 401, description = "Invalid or missing API key"),
        (status = 403, description = "Not authorized for this user"),
    ),
)]
pub async fn get_value_settings(
    Query(query): Query<ValueUserQuery>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<ValueSettingsResponse>, AppError> {
    authorize(&state, &headers, query.user_id).await?;
    Ok(Json(settings_response(value4value::load_settings(&state, query.user_id).await?)))
}

#[utoipa::path(
    put,
    path = "/value/settings",
    tag = "value4value",
    summary = "Save Value4Value settings",
    request_body = ValueSettingsBody,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Saved configuration. The connection string is redacted.", body = ValueSettingsResponse),
        (status = 400, description = "Invalid wallet string or limits"),
        (status = 401, description = "Invalid or missing API key"),
        (status = 403, description = "Not authorized for this user"),
    ),
)]
pub async fn update_value_settings(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(body): Json<ValueSettingsBody>,
) -> Result<Json<ValueSettingsResponse>, AppError> {
    authorize(&state, &headers, body.user_id).await?;
    let view = value4value::save_settings(
        &state,
        SettingsUpdate {
            user_id: body.user_id,
            enabled: body.enabled,
            secret: body.secret,
            clear_secret: body.clear_secret,
            max_boost_sats: body.max_boost_sats,
            stream_sats_per_minute: body.stream_sats_per_minute,
            daily_limit_sats: body.daily_limit_sats,
        },
    )
    .await?;
    Ok(Json(settings_response(view)))
}

#[utoipa::path(
    post,
    path = "/value/test",
    tag = "value4value",
    summary = "Read the wallet balance",
    request_body = ValueTestBody,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Balance probe. ok is false when the wallet could not be reached. No payment is sent.", body = ValueTestResponse),
        (status = 400, description = "The supplied wallet string is not a nostr+walletconnect URI"),
        (status = 401, description = "Invalid or missing API key"),
        (status = 403, description = "Not authorized for this user"),
    ),
)]
pub async fn test_value_wallet(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(body): Json<ValueTestBody>,
) -> Result<Json<ValueTestResponse>, AppError> {
    authorize(&state, &headers, body.user_id).await?;
    let report = value4value::test_wallet(&state, body.user_id, body.secret.as_deref()).await?;
    Ok(Json(ValueTestResponse {
        ok: report.ok,
        balance_sats: report.balance_sats,
        detail: report.detail,
    }))
}

#[utoipa::path(
    get,
    path = "/value/episode",
    tag = "value4value",
    summary = "Whether an episode can receive Value4Value sats",
    params(ValueEpisodeQuery),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Payable when the stored episode or channel value block has a keysend or lightning-address recipient.", body = ValuePreviewResponse),
        (status = 401, description = "Invalid or missing API key"),
        (status = 403, description = "Not authorized for this user"),
        (status = 404, description = "Episode not found for this user"),
    ),
)]
pub async fn preview_value_episode(
    Query(query): Query<ValueEpisodeQuery>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<ValuePreviewResponse>, AppError> {
    authorize(&state, &headers, query.user_id).await?;
    let preview = value4value::preview_episode(&state, query.user_id, query.episode_id).await?;
    Ok(Json(ValuePreviewResponse {
        payable: preview.payable,
        suggested_sats: preview.suggested_sats,
        recipient_count: preview.recipient_count,
        remote_feed_not_loaded: preview.remote_feed_not_loaded,
    }))
}

#[utoipa::path(
    post,
    path = "/value/boost",
    tag = "value4value",
    summary = "Send a one-time boost in sats",
    request_body = ValueBoostBody,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Payment attempt. sent is false when every recipient failed. Partial success lists both paid and failed shares. This spends real sats.", body = ValueSendResponse),
        (status = 400, description = "Feature off, over the max boost, over the daily limit, or nothing to pay"),
        (status = 401, description = "Invalid or missing API key"),
        (status = 403, description = "Not authorized for this user"),
        (status = 404, description = "Episode not found for this user"),
    ),
)]
pub async fn boost_value(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(body): Json<ValueBoostBody>,
) -> Result<Json<ValueSendResponse>, AppError> {
    authorize(&state, &headers, body.user_id).await?;
    let report = value4value::send_boost(
        &state,
        body.user_id,
        body.episode_id,
        body.sats,
        &body.message,
        &body.sender_name,
        body.position_seconds,
    )
    .await?;
    Ok(Json(send_response(report)))
}

#[utoipa::path(
    post,
    path = "/value/stream",
    tag = "value4value",
    summary = "Send the per-minute streaming amount for one playback minute",
    request_body = ValueStreamBody,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Always HTTP 200. sent is false when streaming is off, the minute was already paid, or the wallet declined. Players should stay quiet.", body = ValueSendResponse),
        (status = 401, description = "Invalid or missing API key"),
        (status = 403, description = "Not authorized for this user"),
    ),
)]
pub async fn stream_value(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(body): Json<ValueStreamBody>,
) -> Result<Json<ValueSendResponse>, AppError> {
    authorize(&state, &headers, body.user_id).await?;
    let report = value4value::send_stream(&state, body.user_id, body.episode_id, body.position_seconds).await?;
    Ok(Json(send_response(report)))
}
