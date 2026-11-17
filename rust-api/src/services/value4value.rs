//! Value4Value: opt-in Lightning boosts and streaming sats.
//!
//! The wallet is the user's own Nostr Wallet Connect string. It is encrypted
//! before it is stored and is never returned by the API. Sends stay off until
//! the user turns the feature on and sets a daily limit above zero.
//!
//! Recipients come from the stored episode value block, or the channel block
//! when the episode has none. The client cannot supply addresses. Keysend
//! (`node`) and lightning-address recipients are paid. Other types are skipped
//! and their split is not given to anyone else. A remote valueTimeSplit pays the
//! recipients inlined on the split. A split that only names a `podcast:remoteItem`
//! pays the recipients of that item's value block, read from the remote feed (the
//! public-URL guard applies) and cached. When that block cannot be found the sats
//! stay with the local recipients.
//!
//! Tests use a mock wallet. Nothing in the unit tests dials a relay or pays an invoice.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, PoisonError};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::{NaiveDate, Utc};
use nostr::nips::nip47::{
    KeysendTLVRecord, NostrWalletConnectUri, PayInvoiceRequest, PayKeysendRequest,
};
use serde::Deserialize;
use serde_json::json;
use sqlx::Row;

use crate::database::dialect::dialect_match;
use crate::database::DatabasePool;
use crate::error::{AppError, AppResult};
use crate::services::podcasting2::{self, RemoteItem, ValueBlock, ValueRecipient, ValueTimeSplit};
use crate::services::url_guard::{ensure_safe_public_url_async, guarded_redirect_policy};
use crate::AppState;

/// Podcasting 2.0 boostagram TLV (UTF-8 JSON, hex-encoded on the wire).
pub const BOOSTAGRAM_TLV: u64 = 7_629_169;
/// Keysend preimage TLV. The same 32 bytes are also sent in the NWC preimage field.
pub const KEYSEND_PREIMAGE_TLV: u64 = 5_482_373_484;
/// 100% expressed in hundredths of a percent, so "10.5" is 1_050.
const FULL_SPLIT: i64 = 10_000;
/// One payment, one limit, and one stream rate cannot exceed this many sats.
pub const MAX_SATS: i64 = 2_000_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayKind {
    Keysend,
    LnAddress,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Share {
    pub name: String,
    pub address: String,
    pub kind: PayKind,
    pub sats: u64,
    pub custom_key: Option<u64>,
    pub custom_value: Option<String>,
    pub fee: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedRecipient {
    pub name: String,
    pub kind: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Allocation {
    pub shares: Vec<Share>,
    pub skipped: Vec<SkippedRecipient>,
    pub unallocated_sats: u64,
    /// A matching time split named a remote item and did not include recipients, and that
    /// item's value block was not found. The split's sats stayed with the local recipients.
    pub remote_feed_not_loaded: bool,
}

#[derive(Debug, Clone)]
pub struct EpisodeMeta {
    pub podcast: String,
    pub feed_id: String,
    pub feed_url: String,
    pub episode: String,
    pub episode_guid: String,
    pub podcast_id: i32,
    pub ts_seconds: i64,
    pub sender_name: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaidShare {
    pub name: String,
    pub sats: u64,
    pub preimage: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailedShare {
    pub name: String,
    pub sats: u64,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendReport {
    pub sent: bool,
    pub reason: String,
    pub sats_sent: u64,
    pub paid: Vec<PaidShare>,
    pub failed: Vec<FailedShare>,
    pub skipped: Vec<SkippedRecipient>,
    pub unallocated_sats: u64,
    pub remote_feed_not_loaded: bool,
}

impl SendReport {
    fn quiet(reason: impl Into<String>) -> Self {
        Self {
            sent: false,
            reason: reason.into(),
            sats_sent: 0,
            paid: Vec::new(),
            failed: Vec::new(),
            skipped: Vec::new(),
            unallocated_sats: 0,
            remote_feed_not_loaded: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct KeysendPayment {
    pub pubkey: String,
    pub amount_msat: u64,
    pub preimage_hex: String,
    pub tlv: Vec<(u64, String)>,
}

#[async_trait]
pub trait LightningWallet: Send + Sync {
    async fn keysend(&self, payment: KeysendPayment) -> Result<String, String>;
    async fn pay_invoice(&self, invoice: &str, amount_msat: u64) -> Result<String, String>;
    async fn balance_msat(&self) -> Result<u64, String>;
}

#[async_trait]
pub trait LnurlHttp: Send + Sync {
    async fn get_text(&self, url: &str) -> Result<String, String>;
}

#[derive(Debug, Clone)]
pub struct WalletSettings {
    pub enabled: bool,
    pub has_secret: bool,
    pub secret_encrypted: Option<String>,
    pub max_boost_sats: i64,
    pub stream_sats_per_minute: i64,
    pub daily_limit_sats: i64,
    pub spent_today_sats: i64,
}

#[derive(Debug, Clone)]
pub struct SettingsUpdate {
    pub user_id: i32,
    pub enabled: bool,
    pub secret: Option<String>,
    pub clear_secret: bool,
    pub max_boost_sats: i64,
    pub stream_sats_per_minute: i64,
    pub daily_limit_sats: i64,
}

#[derive(Debug, Clone)]
struct StoredWallet {
    enabled: bool,
    secret_encrypted: Option<String>,
    max_boost_sats: i64,
    stream_sats_per_minute: i64,
    daily_limit_sats: i64,
    spent_day_sats: i64,
    spent_day: Option<NaiveDate>,
}

/// A wallet string is accepted only when it parses as NWC. Parsing does not connect.
pub fn validate_nwc_uri(raw: &str) -> Result<(), String> {
    NostrWalletConnectUri::parse(raw.trim())
        .map(|_| ())
        .map_err(|_| "wallet string must be a nostr+walletconnect URI".to_string())
}

pub fn playback_seconds(raw: i64) -> i64 {
    raw.clamp(0, MAX_SATS)
}

pub fn minute_index(seconds: i64) -> i32 {
    (playback_seconds(seconds) / 60) as i32
}

/// Hundredths of a percent. "90" -> 9000, "10.5" -> 1050. Garbage and negatives are 0.
pub fn parse_split_hundredths(raw: &str) -> i64 {
    let s = raw.trim().trim_end_matches('%').trim();
    if s.is_empty() || s.starts_with('-') {
        return 0;
    }
    let (whole, frac) = s.split_once('.').unwrap_or((s, ""));
    if whole.is_empty() || !whole.chars().all(|c| c.is_ascii_digit()) {
        return 0;
    }
    let whole: i64 = whole.parse().unwrap_or(0);
    let mut frac_digits = String::new();
    for c in frac.chars() {
        if c.is_ascii_digit() {
            frac_digits.push(c);
        } else {
            break;
        }
    }
    while frac_digits.len() < 2 {
        frac_digits.push('0');
    }
    let frac_two: i64 = frac_digits[..2].parse().unwrap_or(0);
    whole.saturating_mul(100).saturating_add(frac_two)
}

fn share_of(total: u64, weight: i64, basis: i64) -> u64 {
    if total == 0 || weight <= 0 || basis <= 0 {
        return 0;
    }
    (total as u128 * weight as u128 / basis as u128) as u64
}

fn scale_down_to(amounts: &mut [u64], cap: u64) {
    let sum: u128 = amounts.iter().map(|a| *a as u128).sum();
    if sum == 0 || sum <= cap as u128 {
        return;
    }
    let mut running = 0u64;
    let last = amounts.len() - 1;
    for (i, amount) in amounts.iter_mut().enumerate() {
        if i == last {
            *amount = cap.saturating_sub(running);
        } else {
            let scaled = (*amount as u128 * cap as u128 / sum) as u64;
            *amount = scaled;
            running = running.saturating_add(scaled);
        }
    }
}

/// A compressed node pubkey is 33 bytes, written as 66 hex characters.
/// Some feeds label that key `lnaddress` (No Agenda's Fountain split does).
fn is_node_pubkey(address: &str) -> bool {
    let address = address.trim();
    address.len() == 66 && address.chars().all(|c| c.is_ascii_hexdigit())
}

pub fn classify_recipient(recipient: &ValueRecipient) -> Option<PayKind> {
    let address = recipient.address.trim();
    if address.is_empty() {
        return None;
    }
    match recipient.kind.trim().to_ascii_lowercase().as_str() {
        "node" | "keysend" | "pubkey" => Some(PayKind::Keysend),
        "lnaddress" | "lightning" | "lnurl" if is_node_pubkey(address) => Some(PayKind::Keysend),
        "lnaddress" | "lightning" | "lnurl" => Some(PayKind::LnAddress),
        _ => None,
    }
}

fn custom_tlv_key(recipient: &ValueRecipient) -> Option<u64> {
    let raw = recipient.custom_key.as_deref()?.trim();
    if raw.is_empty() {
        return None;
    }
    raw.parse().ok()
}

fn blocks_for<'a>(episode: &'a [ValueBlock], channel: &'a [ValueBlock]) -> &'a [ValueBlock] {
    if episode.is_empty() {
        channel
    } else {
        episode
    }
}

fn is_lightning_block(block: &ValueBlock) -> bool {
    let kind = block.kind.to_ascii_lowercase();
    let method = block.method.to_ascii_lowercase();
    kind == "lightning" || method == "lightning" || method.contains("keysend")
}

pub fn select_block<'a>(episode: &'a [ValueBlock], channel: &'a [ValueBlock]) -> Option<&'a ValueBlock> {
    let blocks = blocks_for(episode, channel);
    blocks
        .iter()
        .find(|block| is_lightning_block(block))
        .or_else(|| blocks.first())
}

pub fn block_is_payable(block: &ValueBlock) -> bool {
    block
        .recipients
        .iter()
        .chain(block.time_splits.iter().flat_map(|split| split.recipients.iter()))
        .any(|recipient| classify_recipient(recipient).is_some())
}

pub fn suggested_sats(block: &ValueBlock) -> Option<i64> {
    let raw = block.suggested.as_deref()?.trim();
    if raw.is_empty() {
        return None;
    }
    // A decimal is BTC (Podcasting 2.0). A whole number is already sats.
    let parsed = if raw.contains('.') {
        btc_decimal_to_sats(raw)?
    } else {
        let digits: String = raw.chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() {
            return None;
        }
        digits.parse::<i64>().ok()?
    };
    if parsed <= 0 {
        None
    } else {
        Some(parsed.min(MAX_SATS))
    }
}

/// Convert a BTC decimal to sats with integer arithmetic. 1 BTC = 100_000_000 sats.
/// Eight fractional digits are whole sats; the next digit rounds half up.
fn btc_decimal_to_sats(raw: &str) -> Option<i64> {
    let raw = raw.trim();
    if raw.starts_with('-') || raw.starts_with('+') {
        return None;
    }
    let (whole, frac) = raw.split_once('.')?;
    if whole.is_empty() || !whole.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let mut frac_digits = String::new();
    for c in frac.chars() {
        if c.is_ascii_digit() {
            frac_digits.push(c);
        } else {
            break;
        }
    }
    let whole: i128 = whole.parse().ok()?;
    while frac_digits.len() < 9 {
        frac_digits.push('0');
    }
    let sats_frac: i128 = frac_digits[..8].parse().ok()?;
    let mut sats = whole.saturating_mul(100_000_000).saturating_add(sats_frac);
    if frac_digits.as_bytes().get(8).copied().unwrap_or(b'0') >= b'5' {
        sats = sats.saturating_add(1);
    }
    if sats <= 0 || sats > i64::MAX as i128 {
        return None;
    }
    Some(sats as i64)
}

fn parse_millis(raw: &str) -> Option<i64> {
    let s = raw.trim();
    if s.is_empty() || s.starts_with('-') {
        return None;
    }
    let (whole, frac) = s.split_once('.').unwrap_or((s, ""));
    if whole.is_empty() || !whole.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let whole: i64 = whole.parse().ok()?;
    let mut frac_millis = 0i64;
    let mut place = 100i64;
    for c in frac.chars().take(3) {
        if !c.is_ascii_digit() {
            return None;
        }
        frac_millis += (c as i64 - '0' as i64) * place;
        place /= 10;
    }
    Some(whole.saturating_mul(1000).saturating_add(frac_millis))
}

fn matching_split(block: &ValueBlock, position_seconds: i64) -> Option<&ValueTimeSplit> {
    let at = playback_seconds(position_seconds).saturating_mul(1000);
    block.time_splits.iter().find(|split| {
        let Some(start) = parse_millis(&split.start_time) else {
            return false;
        };
        let Some(duration) = parse_millis(&split.duration) else {
            return false;
        };
        if duration <= 0 {
            return false;
        }
        at >= start && at < start.saturating_add(duration)
    })
}

fn share_from(recipient: &ValueRecipient, sats: u64, fee: bool) -> Option<Share> {
    let kind = classify_recipient(recipient)?;
    Some(Share {
        name: recipient.name.clone(),
        address: recipient.address.trim().to_string(),
        kind,
        sats,
        custom_key: custom_tlv_key(recipient),
        custom_value: recipient
            .custom_value
            .as_ref()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty()),
        fee,
    })
}

fn split_portion(
    portion: u64,
    recipients: &[ValueRecipient],
    fees: bool,
) -> (Vec<Share>, Vec<SkippedRecipient>, u64) {
    if portion == 0 {
        return (Vec::new(), Vec::new(), 0);
    }
    let mut payable: Vec<(&ValueRecipient, i64)> = Vec::new();
    let mut skipped = Vec::new();
    let mut payable_weight = 0i64;
    let mut all_weight = 0i64;
    for recipient in recipients {
        if recipient.fee.unwrap_or(false) != fees {
            continue;
        }
        let weight = parse_split_hundredths(&recipient.split);
        all_weight += weight;
        if classify_recipient(recipient).is_some() {
            payable_weight += weight;
            payable.push((recipient, weight));
        } else {
            skipped.push(SkippedRecipient {
                name: recipient.name.clone(),
                kind: recipient.kind.clone(),
                reason: if recipient.address.trim().is_empty() {
                    "missing address".to_string()
                } else {
                    "unsupported recipient type".to_string()
                },
            });
        }
    }
    if payable.is_empty() || payable_weight <= 0 || all_weight <= 0 {
        return (Vec::new(), skipped, portion);
    }
    let pool = share_of(portion, payable_weight, all_weight);
    let unallocated = portion.saturating_sub(pool);
    let mut shares = Vec::new();
    let mut assigned = 0u64;
    let last = payable.len() - 1;
    for (index, (recipient, weight)) in payable.iter().enumerate() {
        let sats = if index == last {
            pool.saturating_sub(assigned)
        } else {
            share_of(pool, *weight, payable_weight)
        };
        assigned = assigned.saturating_add(sats);
        if sats > 0 {
            if let Some(share) = share_from(recipient, sats, fees) {
                shares.push(share);
            }
        }
    }
    (shares, skipped, unallocated)
}

fn fee_split(total: u64, recipients: &[ValueRecipient]) -> (Vec<Share>, Vec<SkippedRecipient>, u64, u64) {
    let mut entries: Vec<(Option<&ValueRecipient>, u64)> = Vec::new();
    let mut skipped = Vec::new();
    for recipient in recipients.iter().filter(|r| r.fee.unwrap_or(false)) {
        let raw = share_of(total, parse_split_hundredths(&recipient.split), FULL_SPLIT);
        if classify_recipient(recipient).is_some() {
            entries.push((Some(recipient), raw));
        } else {
            skipped.push(SkippedRecipient {
                name: recipient.name.clone(),
                kind: recipient.kind.clone(),
                reason: if recipient.address.trim().is_empty() {
                    "missing address".to_string()
                } else {
                    "unsupported recipient type".to_string()
                },
            });
            entries.push((None, raw));
        }
    }
    if entries.is_empty() {
        return (Vec::new(), skipped, 0, 0);
    }
    let mut amounts: Vec<u64> = entries.iter().map(|(_, amount)| *amount).collect();
    scale_down_to(&mut amounts, total);
    let mut shares = Vec::new();
    let mut paid = 0u64;
    let mut withheld = 0u64;
    for ((recipient, _), amount) in entries.iter().zip(amounts) {
        match recipient {
            Some(recipient) if amount > 0 => {
                if let Some(share) = share_from(recipient, amount, true) {
                    paid = paid.saturating_add(amount);
                    shares.push(share);
                } else {
                    withheld = withheld.saturating_add(amount);
                }
            }
            Some(_) => {}
            None => withheld = withheld.saturating_add(amount),
        }
    }
    (shares, skipped, paid, withheld)
}

/// A remote item's value block paid out of `portion`: its own fees first, then the rest by weight.
/// Returns the shares, the skipped recipients, and the sats that were held back.
fn remote_block_shares(
    portion: u64,
    recipients: &[ValueRecipient],
) -> (Vec<Share>, Vec<SkippedRecipient>, u64) {
    let (mut shares, mut skipped, fee_paid, fee_withheld) = fee_split(portion, recipients);
    let rest = portion.saturating_sub(fee_paid).saturating_sub(fee_withheld);
    let (paid, other_skipped, unallocated) = split_portion(rest, recipients, false);
    shares.extend(paid);
    skipped.extend(other_skipped);
    (shares, skipped, unallocated.saturating_add(fee_withheld))
}

/// Split `total_sats` across the value block at `position_seconds`.
///
/// Fees are a percent of the original total and never sum past it. A time split
/// that contains the playhead takes `remotePercentage` of what remains (100 when
/// that split lists recipients and omits the percentage). Those sats go to the
/// inlined recipients. A split that only names a remote item pays `remote`, the
/// recipients of that item's own value block, out of the same portion: its fees
/// first, then the rest by weight. Without `remote` (not looked up, or not found)
/// that remainder stays on the local recipients and `remote_feed_not_loaded` is set.
pub fn allocate(
    total_sats: u64,
    block: &ValueBlock,
    position_seconds: Option<i64>,
    remote: Option<&[ValueRecipient]>,
) -> Allocation {
    let (fee_shares, mut skipped, fee_paid, fee_withheld) = fee_split(total_sats, &block.recipients);
    let mut remainder = total_sats.saturating_sub(fee_paid).saturating_sub(fee_withheld);
    let mut remote_feed_not_loaded = false;
    let mut remote_shares = Vec::new();
    let mut withheld_remote = 0u64;
    if let Some(position) = position_seconds {
        if let Some(split) = matching_split(block, position) {
            let remote_block = match (&split.remote_item, remote) {
                (Some(_), Some(recipients)) if split.recipients.is_empty() && !recipients.is_empty() => {
                    Some(recipients)
                }
                _ => None,
            };
            if split.recipients.is_empty() && remote_block.is_none() {
                remote_feed_not_loaded = split.remote_item.is_some();
            } else {
                let pct = split
                    .remote_percentage
                    .as_deref()
                    .filter(|raw| !raw.trim().is_empty())
                    .map(parse_split_hundredths)
                    .unwrap_or(FULL_SPLIT)
                    .clamp(0, FULL_SPLIT);
                let remote_portion = share_of(remainder, pct, FULL_SPLIT);
                let (shares, remote_skipped, remote_unallocated) = match remote_block {
                    Some(recipients) => remote_block_shares(remote_portion, recipients),
                    None => split_portion(remote_portion, &split.recipients, false),
                };
                remainder = remainder.saturating_sub(remote_portion);
                skipped.extend(remote_skipped);
                remote_shares = shares;
                withheld_remote = remote_unallocated;
            }
        }
    }
    let (local_shares, local_skipped, unallocated) =
        split_portion(remainder, &block.recipients, false);
    skipped.extend(local_skipped);
    let mut shares = fee_shares;
    shares.extend(remote_shares);
    shares.extend(local_shares);
    Allocation {
        shares,
        skipped,
        unallocated_sats: unallocated
            .saturating_add(withheld_remote)
            .saturating_add(fee_withheld),
        remote_feed_not_loaded,
    }
}

/// Even-length hex is sent as-is. Anything else is the hex of its UTF-8 bytes.
pub fn tlv_hex(value: &str) -> String {
    let trimmed = value.trim();
    if !trimmed.is_empty()
        && trimmed.len() % 2 == 0
        && trimmed.chars().all(|c| c.is_ascii_hexdigit())
    {
        trimmed.to_ascii_lowercase()
    } else {
        hex::encode(trimmed.as_bytes())
    }
}

pub fn boostagram_json(meta: &EpisodeMeta, action: &str, recipient_name: &str, value_msat: u64, total_msat: u64) -> String {
    json!({
        "podcast": meta.podcast,
        "feedID": meta.feed_id,
        "url": meta.feed_url,
        "episode": meta.episode,
        "episode_guid": meta.episode_guid,
        "ts": meta.ts_seconds,
        "action": action,
        "app_name": "PinePods",
        "value_msat": value_msat,
        "value_msat_total": total_msat,
        "name": recipient_name,
        "sender_name": meta.sender_name,
        "message": meta.message,
    })
    .to_string()
}

fn random_preimage() -> String {
    let mut bytes = [0u8; 32];
    rand::RngExt::fill(&mut rand::rng(), &mut bytes);
    hex::encode(bytes)
}

pub fn keysend_payment(share: &Share, meta: &EpisodeMeta, action: &str, total_sats: u64) -> KeysendPayment {
    let preimage = random_preimage();
    let body = boostagram_json(meta, action, &share.name, share.sats.saturating_mul(1000), total_sats.saturating_mul(1000));
    let mut tlv = vec![
        (KEYSEND_PREIMAGE_TLV, preimage.clone()),
        (BOOSTAGRAM_TLV, hex::encode(body.as_bytes())),
    ];
    if let (Some(key), Some(value)) = (share.custom_key, share.custom_value.as_deref()) {
        tlv.push((key, tlv_hex(value)));
    }
    KeysendPayment {
        pubkey: share.address.clone(),
        amount_msat: share.sats.saturating_mul(1000),
        preimage_hex: preimage,
        tlv,
    }
}

pub fn lnurlp_url(address: &str) -> Result<String, String> {
    let address = address.trim();
    let Some((user, domain)) = address.split_once('@') else {
        return Err("lightning address needs a name and a domain".to_string());
    };
    if user.is_empty()
        || domain.is_empty()
        || !user
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '-'))
        || !domain
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
        || !domain.contains('.')
        || domain.starts_with('.')
        || domain.ends_with('.')
    {
        return Err("lightning address is not usable".to_string());
    }
    Ok(format!("https://{domain}/.well-known/lnurlp/{user}"))
}

pub fn callback_url(callback: &str, amount_msat: u64, comment: Option<&str>) -> Result<String, String> {
    let mut url = url::Url::parse(callback).map_err(|_| "lightning callback URL is not valid".to_string())?;
    if url.scheme() != "https" && url.scheme() != "http" {
        return Err("lightning callback must be an http(s) URL".to_string());
    }
    {
        let mut pairs = url.query_pairs_mut();
        pairs.append_pair("amount", &amount_msat.to_string());
        if let Some(comment) = comment.filter(|c| !c.is_empty()) {
            pairs.append_pair("comment", comment);
        }
    }
    Ok(url.to_string())
}

fn fit_comment(boostagram: &str, message: &str, allowed: Option<u64>) -> Option<String> {
    let Some(allowed) = allowed.filter(|n| *n > 0) else {
        return None;
    };
    let limit = allowed as usize;
    if boostagram.chars().count() <= limit {
        return Some(boostagram.to_string());
    }
    let short = if message.trim().is_empty() {
        "PinePods".to_string()
    } else {
        message.trim().to_string()
    };
    Some(short.chars().take(limit).collect())
}

#[derive(Debug, Deserialize)]
struct LnurlPay {
    callback: Option<String>,
    #[serde(rename = "commentAllowed")]
    comment_allowed: Option<u64>,
    #[serde(rename = "minSendable")]
    min_sendable: Option<u64>,
    #[serde(rename = "maxSendable")]
    max_sendable: Option<u64>,
    status: Option<String>,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LnurlInvoice {
    pr: Option<String>,
    status: Option<String>,
    reason: Option<String>,
}

async fn pay_lnaddress(
    share: &Share,
    meta: &EpisodeMeta,
    action: &str,
    total_sats: u64,
    wallet: &dyn LightningWallet,
    http: &dyn LnurlHttp,
) -> Result<String, String> {
    let url = lnurlp_url(&share.address)?;
    let body = http.get_text(&url).await?;
    let info: LnurlPay = serde_json::from_str(&body).map_err(|_| "lightning address lookup was not JSON".to_string())?;
    if info.status.as_deref() == Some("ERROR") {
        return Err(info.reason.unwrap_or_else(|| "lightning address rejected the lookup".to_string()));
    }
    let Some(callback) = info.callback.filter(|c| !c.trim().is_empty()) else {
        return Err("lightning address did not return a callback".to_string());
    };
    let msat = share.sats.saturating_mul(1000);
    if let Some(min) = info.min_sendable {
        if msat < min {
            return Err(format!("amount is below the lightning address minimum ({min} msats)"));
        }
    }
    if let Some(max) = info.max_sendable {
        if msat > max {
            return Err(format!("amount is above the lightning address maximum ({max} msats)"));
        }
    }
    let body = boostagram_json(meta, action, &share.name, msat, total_sats.saturating_mul(1000));
    let comment = fit_comment(&body, &meta.message, info.comment_allowed);
    let callback = callback_url(&callback, msat, comment.as_deref())?;
    let invoice_body = http.get_text(&callback).await?;
    let invoice: LnurlInvoice =
        serde_json::from_str(&invoice_body).map_err(|_| "lightning invoice response was not JSON".to_string())?;
    if invoice.status.as_deref() == Some("ERROR") {
        return Err(invoice.reason.unwrap_or_else(|| "lightning address rejected the invoice".to_string()));
    }
    let Some(pr) = invoice.pr.filter(|p| !p.trim().is_empty()) else {
        return Err("lightning address did not return an invoice".to_string());
    };
    wallet.pay_invoice(pr.trim(), msat).await
}

pub async fn execute_payments(
    shares: &[Share],
    meta: &EpisodeMeta,
    action: &str,
    total_sats: u64,
    wallet: &dyn LightningWallet,
    http: &dyn LnurlHttp,
) -> (Vec<PaidShare>, Vec<FailedShare>) {
    let mut paid = Vec::new();
    let mut failed = Vec::new();
    for share in shares.iter().filter(|share| share.sats > 0) {
        let result = match share.kind {
            PayKind::Keysend => {
                let payment = keysend_payment(share, meta, action, total_sats);
                wallet.keysend(payment).await
            }
            PayKind::LnAddress => pay_lnaddress(share, meta, action, total_sats, wallet, http).await,
        };
        match result {
            Ok(preimage) => paid.push(PaidShare {
                name: share.name.clone(),
                sats: share.sats,
                preimage,
            }),
            Err(error) => failed.push(FailedShare {
                name: share.name.clone(),
                sats: share.sats,
                error,
            }),
        }
    }
    (paid, failed)
}

/// New spent total when a reserve of `amount` is allowed. `None` refuses the send.
pub fn spent_after_reserve(
    enabled: bool,
    daily_limit: i64,
    spent: i64,
    spent_day: Option<NaiveDate>,
    today: NaiveDate,
    amount: i64,
) -> Option<i64> {
    if !enabled || daily_limit <= 0 || amount <= 0 || amount > MAX_SATS {
        return None;
    }
    let spent = if spent_day == Some(today) { spent.max(0) } else { 0 };
    let next = spent.saturating_add(amount);
    if next > daily_limit || next > MAX_SATS {
        None
    } else {
        Some(next)
    }
}

pub fn clip_text(raw: &str, max_chars: usize) -> String {
    raw.trim().chars().take(max_chars).collect()
}

fn limits_ok(value: i64) -> bool {
    (0..=MAX_SATS).contains(&value)
}

fn today_utc() -> NaiveDate {
    Utc::now().date_naive()
}

fn view_from(stored: &StoredWallet) -> WalletSettings {
    let spent_today = if stored.spent_day == Some(today_utc()) {
        stored.spent_day_sats.max(0)
    } else {
        0
    };
    WalletSettings {
        enabled: stored.enabled,
        has_secret: stored
            .secret_encrypted
            .as_deref()
            .map(|s| !s.is_empty())
            .unwrap_or(false),
        secret_encrypted: stored.secret_encrypted.clone(),
        max_boost_sats: stored.max_boost_sats,
        stream_sats_per_minute: stored.stream_sats_per_minute,
        daily_limit_sats: stored.daily_limit_sats,
        spent_today_sats: spent_today,
    }
}

pub async fn load_settings(state: &AppState, user_id: i32) -> AppResult<WalletSettings> {
    let stored = read_settings(&state.db_pool, user_id).await?;
    Ok(view_from(&stored))
}

pub async fn save_settings(state: &AppState, update: SettingsUpdate) -> AppResult<WalletSettings> {
    if !limits_ok(update.max_boost_sats)
        || !limits_ok(update.stream_sats_per_minute)
        || !limits_ok(update.daily_limit_sats)
    {
        return Err(AppError::bad_request(
            "sats limits must be between 0 and 2000000000",
        ));
    }
    let existing = read_settings(&state.db_pool, update.user_id).await?;
    let mut secret = existing.secret_encrypted.clone();
    if update.clear_secret {
        secret = None;
    }
    if let Some(raw) = update.secret.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        validate_nwc_uri(raw).map_err(AppError::bad_request)?;
        secret = Some(state.db_pool.encrypt_password(raw).await?);
    }
    if update.enabled && secret.as_deref().unwrap_or("").is_empty() {
        return Err(AppError::bad_request(
            "a wallet connection string is required to turn Value for Value on",
        ));
    }
    let stored = StoredWallet {
        enabled: update.enabled,
        secret_encrypted: secret,
        max_boost_sats: update.max_boost_sats,
        stream_sats_per_minute: update.stream_sats_per_minute,
        daily_limit_sats: update.daily_limit_sats,
        spent_day_sats: existing.spent_day_sats,
        spent_day: existing.spent_day,
    };
    write_settings(&state.db_pool, update.user_id, &stored).await?;
    Ok(view_from(&stored))
}

pub struct BalanceReport {
    pub ok: bool,
    pub balance_sats: u64,
    pub detail: String,
}

pub async fn test_wallet(state: &AppState, user_id: i32, secret_override: Option<&str>) -> AppResult<BalanceReport> {
    let secret = if let Some(raw) = secret_override.map(str::trim).filter(|s| !s.is_empty()) {
        validate_nwc_uri(raw).map_err(AppError::bad_request)?;
        raw.to_string()
    } else {
        let stored = read_settings(&state.db_pool, user_id).await?;
        let Some(enc) = stored.secret_encrypted.filter(|s| !s.is_empty()) else {
            return Ok(BalanceReport {
                ok: false,
                balance_sats: 0,
                detail: "no wallet string is saved".to_string(),
            });
        };
        state.db_pool.decrypt_password(&enc).await?
    };
    let wallet = match NwcWallet::connect(&secret) {
        Ok(wallet) => wallet,
        Err(err) => {
            return Ok(BalanceReport {
                ok: false,
                balance_sats: 0,
                detail: err,
            })
        }
    };
    match wallet.balance_msat().await {
        Ok(msat) => Ok(BalanceReport {
            ok: true,
            balance_sats: msat / 1000,
            detail: String::new(),
        }),
        Err(err) => Ok(BalanceReport {
            ok: false,
            balance_sats: 0,
            detail: err,
        }),
    }
}

#[derive(Debug, Clone)]
struct EpisodeRow {
    podcast_id: i32,
    podcast: String,
    feed_id: String,
    feed_url: String,
    episode: String,
    episode_guid: String,
}

pub struct Preview {
    pub payable: bool,
    pub suggested_sats: Option<i64>,
    pub recipient_count: i64,
    pub remote_feed_not_loaded: bool,
}

pub async fn preview_episode(state: &AppState, user_id: i32, episode_id: i32) -> AppResult<Preview> {
    let Some(row) = load_episode(&state.db_pool, user_id, episode_id).await? else {
        return Err(AppError::not_found("episode not found"));
    };
    let (episode_blocks, channel_blocks) = load_blocks(&state.db_pool, episode_id, row.podcast_id).await?;
    let Some(block) = select_block(&episode_blocks, &channel_blocks) else {
        return Ok(Preview {
            payable: false,
            suggested_sats: None,
            recipient_count: 0,
            remote_feed_not_loaded: false,
        });
    };
    let probe = allocate(1_000, block, Some(0), None);
    Ok(Preview {
        payable: block_is_payable(block),
        suggested_sats: suggested_sats(block),
        recipient_count: probe.shares.len() as i64,
        remote_feed_not_loaded: probe.remote_feed_not_loaded,
    })
}

pub async fn send_boost(
    state: &AppState,
    user_id: i32,
    episode_id: i32,
    sats: i64,
    message: &str,
    sender_name: &str,
    position_seconds: i64,
) -> AppResult<SendReport> {
    if sats <= 0 || sats > MAX_SATS {
        return Err(AppError::bad_request("boost amount must be a positive number of sats"));
    }
    let settings = load_settings(state, user_id).await?;
    if !settings.enabled || !settings.has_secret {
        return Err(AppError::bad_request(
            "Value for Value is off. Turn it on and save a wallet string first",
        ));
    }
    if settings.daily_limit_sats <= 0 {
        return Err(AppError::bad_request(
            "set a daily limit above 0 before sending sats",
        ));
    }
    if settings.max_boost_sats <= 0 || sats > settings.max_boost_sats {
        return Err(AppError::bad_request(format!(
            "boost is above the max of {} sats",
            settings.max_boost_sats
        )));
    }
    dispatch(
        state,
        user_id,
        episode_id,
        sats as u64,
        "boost",
        message,
        sender_name,
        position_seconds,
        false,
    )
    .await
}

pub async fn send_stream(
    state: &AppState,
    user_id: i32,
    episode_id: i32,
    position_seconds: i64,
) -> AppResult<SendReport> {
    let settings = load_settings(state, user_id).await?;
    if !settings.enabled || !settings.has_secret || settings.stream_sats_per_minute <= 0 {
        return Ok(SendReport::quiet("streaming is off"));
    }
    if settings.daily_limit_sats <= 0 {
        return Ok(SendReport::quiet("daily limit is 0"));
    }
    dispatch(
        state,
        user_id,
        episode_id,
        settings.stream_sats_per_minute as u64,
        "stream",
        "",
        "",
        position_seconds,
        true,
    )
    .await
}

async fn dispatch(
    state: &AppState,
    user_id: i32,
    episode_id: i32,
    amount: u64,
    action: &str,
    message: &str,
    sender_name: &str,
    position_seconds: i64,
    is_stream: bool,
) -> AppResult<SendReport> {
    let Some(row) = load_episode(&state.db_pool, user_id, episode_id).await? else {
        let reason = "episode not found";
        return if is_stream {
            Ok(SendReport::quiet(reason))
        } else {
            Err(AppError::not_found(reason))
        };
    };
    let (episode_blocks, channel_blocks) = load_blocks(&state.db_pool, episode_id, row.podcast_id).await?;
    let Some(block) = select_block(&episode_blocks, &channel_blocks) else {
        let reason = "this episode has no value block";
        return if is_stream {
            Ok(SendReport::quiet(reason))
        } else {
            Err(AppError::bad_request(reason))
        };
    };
    let http = GuardedHttp;
    let position = playback_seconds(position_seconds);
    let remote_item = matching_split(block, position)
        .filter(|split| split.recipients.is_empty())
        .and_then(|split| split.remote_item.as_ref());
    let remote = match remote_item {
        Some(item) => remote_recipients(state, &http, user_id, item).await,
        None => None,
    };
    let allocation = allocate(amount, block, Some(position), remote.as_deref());
    let to_pay: u64 = allocation.shares.iter().map(|share| share.sats).sum();
    if to_pay == 0 {
        let mut report = SendReport::quiet("nothing to send for this value block");
        report.skipped = allocation.skipped;
        report.unallocated_sats = allocation.unallocated_sats.saturating_add(amount);
        report.remote_feed_not_loaded = allocation.remote_feed_not_loaded;
        return if is_stream {
            Ok(report)
        } else {
            Err(AppError::bad_request(report.reason))
        };
    }
    let minute = minute_index(position_seconds);
    if is_stream && !claim_minute(&state.db_pool, user_id, episode_id, minute).await? {
        return Ok(SendReport::quiet("already sent this minute"));
    }
    if !reserve_budget(&state.db_pool, user_id, to_pay as i64).await? {
        if is_stream {
            let _ = release_minute(&state.db_pool, user_id, episode_id, minute).await;
        }
        let reason = "daily limit reached";
        return if is_stream {
            Ok(SendReport::quiet(reason))
        } else {
            Err(AppError::bad_request(reason))
        };
    }
    let rollback = || async {
        let _ = release_budget(&state.db_pool, user_id, to_pay as i64).await;
        if is_stream {
            let _ = release_minute(&state.db_pool, user_id, episode_id, minute).await;
        }
    };
    let secret = match decrypt_secret(state, user_id).await {
        Ok(secret) => secret,
        Err(err) => {
            rollback().await;
            return Err(err);
        }
    };
    let wallet = match NwcWallet::connect(&secret) {
        Ok(wallet) => wallet,
        Err(err) => {
            rollback().await;
            return Ok(SendReport::quiet(err));
        }
    };
    let meta = EpisodeMeta {
        podcast: row.podcast,
        feed_id: row.feed_id,
        feed_url: row.feed_url,
        episode: row.episode,
        episode_guid: row.episode_guid,
        podcast_id: row.podcast_id,
        ts_seconds: playback_seconds(position_seconds),
        sender_name: clip_text(sender_name, 80),
        message: clip_text(message, 500),
    };
    let (paid, failed) = execute_payments(&allocation.shares, &meta, action, amount, &wallet, &http).await;
    let failed_sats: u64 = failed.iter().map(|item| item.sats).sum();
    if failed_sats > 0 {
        let _ = release_budget(&state.db_pool, user_id, failed_sats as i64).await;
    }
    if paid.is_empty() && is_stream {
        let _ = release_minute(&state.db_pool, user_id, episode_id, minute).await;
    }
    let sats_sent: u64 = paid.iter().map(|item| item.sats).sum();
    let reason = if paid.is_empty() {
        failed
            .first()
            .map(|item| item.error.clone())
            .unwrap_or_else(|| "payment failed".to_string())
    } else {
        String::new()
    };
    Ok(SendReport {
        sent: sats_sent > 0,
        reason,
        sats_sent,
        paid,
        failed,
        skipped: allocation.skipped,
        unallocated_sats: allocation.unallocated_sats,
        remote_feed_not_loaded: allocation.remote_feed_not_loaded,
    })
}

/// Remote value blocks by user and `(feedGuid, itemGuid, feedUrl)`, so a stream does not refetch the
/// remote feed every minute. The user and the `feedUrl` a split names are part of the key, so a feed
/// one user follows, or a URL one feed names, cannot supply the block that another lookup pays.
/// A block that was found is reused for 6 hours, a failed lookup for 10 minutes.
type RemoteValueKey = (i32, String, String, String);
type RemoteValueCache = HashMap<RemoteValueKey, (Instant, Option<Vec<ValueRecipient>>)>;
static REMOTE_VALUE: LazyLock<Mutex<RemoteValueCache>> = LazyLock::new(Default::default);
const REMOTE_VALUE_TTL: Duration = Duration::from_secs(6 * 60 * 60);
const REMOTE_VALUE_MISS_TTL: Duration = Duration::from_secs(10 * 60);
const REMOTE_VALUE_MAX_ENTRIES: usize = 512;

/// `Some` when the lookup was answered recently. The inner `None` is a remembered failure.
fn cached_remote_value(key: &RemoteValueKey) -> Option<Option<Vec<ValueRecipient>>> {
    let cache = REMOTE_VALUE.lock().unwrap_or_else(PoisonError::into_inner);
    let (stored_at, found) = cache.get(key)?;
    let ttl = if found.is_some() { REMOTE_VALUE_TTL } else { REMOTE_VALUE_MISS_TTL };
    (stored_at.elapsed() < ttl).then(|| found.clone())
}

fn store_remote_value(key: RemoteValueKey, found: Option<Vec<ValueRecipient>>) {
    let mut cache = REMOTE_VALUE.lock().unwrap_or_else(PoisonError::into_inner);
    if cache.len() > REMOTE_VALUE_MAX_ENTRIES {
        cache.clear();
    }
    cache.insert(key, (Instant::now(), found));
}

fn is_http_url(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

/// The recipients of `item_guid`'s value block in a remote feed, else the channel's block, which is
/// what an item without its own block uses. `None` when the feed declares a different `podcast:guid`
/// than `feed_guid`, or the block has no recipients. The remote block's own time splits are ignored.
fn recipients_from_feed(xml: &str, feed_guid: &str, item_guid: Option<&str>) -> Option<Vec<ValueRecipient>> {
    let tags = podcasting2::parse(xml);
    if let Some(declared) = tags.podcast_guid.as_deref() {
        if !declared.trim().eq_ignore_ascii_case(feed_guid.trim()) {
            return None;
        }
    }
    let item = item_guid
        .map(str::trim)
        .filter(|guid| !guid.is_empty())
        .and_then(|guid| {
            tags.items
                .iter()
                .find(|candidate| candidate.guid.as_deref().map(str::trim) == Some(guid))
        });
    let item_value: &[ValueBlock] = match item {
        Some(item) => &item.value,
        None => &[],
    };
    let block = select_block(item_value, &tags.extras.value)?;
    (!block.recipients.is_empty()).then(|| block.recipients.clone())
}

/// The feed URL a search-service answer gives for `feed_guid`: `feed` (a by-GUID lookup), else the
/// first of `feeds` whose `podcastGuid` matches. A result that does not carry the GUID is not this feed.
fn feed_url_from_search(body: &serde_json::Value, feed_guid: &str) -> Option<String> {
    let wanted = feed_guid.trim();
    let feed = std::iter::once(&body["feed"])
        .chain(body["feeds"].as_array().into_iter().flatten())
        .find(|feed| {
            feed.get("podcastGuid")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|guid| guid.trim().eq_ignore_ascii_case(wanted))
        })?;
    let url = feed.get("url")?.as_str()?.trim();
    is_http_url(url).then(|| url.to_string())
}

/// Ask the search service (Podcast Index by GUID) where a feed lives. The URL is operator config
/// and may be an internal host, so it is not run through the public-URL guard.
async fn search_feed_url(state: &AppState, feed_guid: &str) -> Option<String> {
    let response = reqwest::Client::new()
        .get(&state.config.api.search_api_url)
        .query(&[("query", feed_guid), ("index", "podcastindex"), ("search_type", "guid")])
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let body: serde_json::Value = response.json().await.ok()?;
    feed_url_from_search(&body, feed_guid)
}

/// Find the remote item's value block: the `feedUrl` its split names, else a podcast this user already
/// follows with that `feedGuid`, else the search service. Then fetch the feed and read the block.
async fn fetch_remote_recipients(
    state: &AppState,
    http: &dyn LnurlHttp,
    user_id: i32,
    feed_guid: &str,
    item_guid: Option<&str>,
    feed_url: Option<&str>,
) -> Option<Vec<ValueRecipient>> {
    let mut url = feed_url.map(str::to_string);
    if url.is_none() {
        url = local_feed_url(&state.db_pool, user_id, feed_guid).await.unwrap_or_else(|err| {
            tracing::warn!("remote value block: looking up feed {feed_guid} failed: {err}");
            None
        });
    }
    if url.is_none() {
        url = search_feed_url(state, feed_guid).await;
    }
    let url = url?;
    let xml = match http.get_text(&url).await {
        Ok(xml) => xml,
        Err(err) => {
            tracing::debug!("remote value block: fetching feed {feed_guid} failed: {err}");
            return None;
        }
    };
    recipients_from_feed(&xml, feed_guid, item_guid)
}

/// The recipients a time split's remote item pays, from the cache or looked up now. `None` leaves
/// the split's sats on the local recipients. A failed lookup never fails the payment, and it is
/// bounded by the timeouts of the requests it makes.
async fn remote_recipients(
    state: &AppState,
    http: &dyn LnurlHttp,
    user_id: i32,
    item: &RemoteItem,
) -> Option<Vec<ValueRecipient>> {
    let feed_guid = item.feed_guid.trim();
    if feed_guid.is_empty() {
        return None;
    }
    let item_guid = item.item_guid.as_deref().map(str::trim).filter(|guid| !guid.is_empty());
    let feed_url = item.feed_url.as_deref().map(str::trim).filter(|url| is_http_url(url));
    let key = (
        user_id,
        feed_guid.to_ascii_lowercase(),
        item_guid.unwrap_or_default().to_string(),
        feed_url.unwrap_or_default().to_string(),
    );
    if let Some(cached) = cached_remote_value(&key) {
        return cached;
    }
    let found = fetch_remote_recipients(state, http, user_id, feed_guid, item_guid, feed_url).await;
    if found.is_none() {
        tracing::info!("no value block found for remote feed {feed_guid}; its split stays with the local recipients");
    }
    store_remote_value(key, found.clone());
    found
}

async fn decrypt_secret(state: &AppState, user_id: i32) -> AppResult<String> {
    let stored = read_settings(&state.db_pool, user_id).await?;
    let Some(enc) = stored.secret_encrypted.filter(|s| !s.is_empty()) else {
        return Err(AppError::bad_request("no wallet string is saved"));
    };
    state.db_pool.decrypt_password(&enc).await
}

pub struct NwcWallet {
    inner: nwc::NostrWalletConnect,
}

impl NwcWallet {
    pub fn connect(uri: &str) -> Result<Self, String> {
        let uri = NostrWalletConnectUri::parse(uri.trim())
            .map_err(|_| "wallet string must be a nostr+walletconnect URI".to_string())?;
        let inner = nwc::NostrWalletConnect::builder(uri)
            .timeout(Duration::from_secs(20))
            .build();
        Ok(Self { inner })
    }
}

#[async_trait]
impl LightningWallet for NwcWallet {
    async fn keysend(&self, payment: KeysendPayment) -> Result<String, String> {
        let request = PayKeysendRequest {
            id: None,
            amount: payment.amount_msat,
            pubkey: payment.pubkey,
            preimage: Some(payment.preimage_hex.clone()),
            tlv_records: payment
                .tlv
                .into_iter()
                .map(|(tlv_type, value)| KeysendTLVRecord { tlv_type, value })
                .collect(),
        };
        let response = self
            .inner
            .pay_keysend(request)
            .await
            .map_err(|err| err.to_string())?;
        Ok(if response.preimage.is_empty() {
            payment.preimage_hex
        } else {
            response.preimage
        })
    }

    async fn pay_invoice(&self, invoice: &str, amount_msat: u64) -> Result<String, String> {
        let mut request = PayInvoiceRequest::new(invoice);
        request.amount = Some(amount_msat);
        let response = self
            .inner
            .pay_invoice(request)
            .await
            .map_err(|err| err.to_string())?;
        Ok(response.preimage)
    }

    async fn balance_msat(&self) -> Result<u64, String> {
        let response = self.inner.get_balance().await.map_err(|err| err.to_string())?;
        Ok(response.balance)
    }
}

/// A feed or lightning lookup larger than this is refused, so a hostile URL cannot fill memory.
const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

pub struct GuardedHttp;

#[async_trait]
impl LnurlHttp for GuardedHttp {
    async fn get_text(&self, url: &str) -> Result<String, String> {
        ensure_safe_public_url_async(url).await?;
        let client = reqwest::Client::builder()
            .redirect(guarded_redirect_policy())
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|err| err.to_string())?;
        let mut response = client.get(url).send().await.map_err(|err| err.to_string())?;
        if !response.status().is_success() {
            return Err(format!("lightning address lookup returned HTTP {}", response.status()));
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|err| err.to_string())? {
            if body.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
                return Err("response too large".to_string());
            }
            body.extend_from_slice(&chunk);
        }
        Ok(String::from_utf8(body).unwrap_or_else(|err| String::from_utf8_lossy(err.as_bytes()).into_owned()))
    }
}

async fn load_episode(db: &DatabasePool, user_id: i32, episode_id: i32) -> AppResult<Option<EpisodeRow>> {
    dialect_match!(db, {
        pg(pool) => {
            let row = q::query(
                r#"SELECT p.podcastid AS podcast_id,
                          e.episodetitle AS episode,
                          COALESCE(e.episodeguid, '') AS episode_guid,
                          p.podcastname AS podcast,
                          COALESCE(p.podcastguid, '') AS feed_id,
                          COALESCE(p.feedurl, '') AS feed_url
                   FROM "Episodes" e
                   JOIN "Podcasts" p ON p.feedid = e.feedid
                   WHERE e.episodeid = $1 AND p.userid = $2
                   LIMIT 1"#,
            )
            .bind(episode_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
            let map_episode_row = |row: &q::Row| -> AppResult<EpisodeRow> {
                Ok(EpisodeRow {
                    podcast_id: row.try_get::<i32, _>("podcast_id")?,
                    episode: row.try_get("episode")?,
                    episode_guid: row.try_get("episode_guid")?,
                    podcast: row.try_get("podcast")?,
                    feed_id: row.try_get("feed_id")?,
                    feed_url: row.try_get("feed_url")?,
                })
            };
            Ok(row.map(|row| map_episode_row(&row)).transpose()?)
        },
        mysql(pool) => {
            let row = sqlx::query(
                r#"SELECT p.PodcastID AS podcast_id,
                          e.EpisodeTitle AS episode,
                          COALESCE(e.EpisodeGUID, '') AS episode_guid,
                          p.PodcastName AS podcast,
                          COALESCE(p.PodcastGUID, '') AS feed_id,
                          COALESCE(p.FeedURL, '') AS feed_url
                   FROM Episodes e
                   JOIN Podcasts p ON p.FeedID = e.FeedID
                   WHERE e.EpisodeID = ? AND p.UserID = ?
                   LIMIT 1"#,
            )
            .bind(episode_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
            Ok(row.map(|row| map_episode_mysql(&row)).transpose()?)
        },
    })
}

fn map_episode_mysql(row: &sqlx::mysql::MySqlRow) -> AppResult<EpisodeRow> {
    Ok(EpisodeRow {
        podcast_id: row.try_get::<i32, _>("podcast_id").or_else(|_| {
            row.try_get::<i64, _>("podcast_id")
                .map(|v| v as i32)
        })?,
        episode: row.try_get("episode")?,
        episode_guid: row.try_get("episode_guid")?,
        podcast: row.try_get("podcast")?,
        feed_id: row.try_get("feed_id")?,
        feed_url: row.try_get("feed_url")?,
    })
}

/// The feed URL of a podcast this user follows, by its `podcast:guid`. Other users' podcasts are not
/// searched: a feed's GUID is whatever its author wrote, so it only identifies the feeds the user chose.
async fn local_feed_url(db: &DatabasePool, user_id: i32, feed_guid: &str) -> AppResult<Option<String>> {
    dialect_match!(db, {
        pg(pool) => {
            let row = q::query(
                r#"SELECT feedurl AS url FROM "Podcasts"
                   WHERE lower(podcastguid) = lower($1) AND userid = $2 AND feedurl IS NOT NULL
                   LIMIT 1"#,
            )
            .bind(feed_guid)
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
            Ok(row.map(|row| row.try_get::<String, _>("url")).transpose()?)
        },
        mysql(pool) => {
            let row = sqlx::query(
                r#"SELECT FeedURL AS url FROM Podcasts
                   WHERE LOWER(PodcastGUID) = LOWER(?) AND UserID = ? AND FeedURL IS NOT NULL
                   LIMIT 1"#,
            )
            .bind(feed_guid)
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
            Ok(row.map(|row| row.try_get::<String, _>("url")).transpose()?)
        },
    })
}

async fn load_blocks(
    db: &DatabasePool,
    episode_id: i32,
    podcast_id: i32,
) -> AppResult<(Vec<ValueBlock>, Vec<ValueBlock>)> {
    let episode = crate::services::people::store::episode_extras(db, episode_id).await?;
    let channel = crate::services::people::store::channel_extras(db, podcast_id).await?;
    Ok((episode.value, channel.value))
}

fn disabled_wallet() -> StoredWallet {
    StoredWallet {
        enabled: false,
        secret_encrypted: None,
        max_boost_sats: 1000,
        stream_sats_per_minute: 0,
        daily_limit_sats: 0,
        spent_day_sats: 0,
        spent_day: None,
    }
}

fn map_wallet(
    enabled: i64,
    secret: Option<String>,
    max_boost: i64,
    stream: i64,
    daily: i64,
    spent: i64,
    spent_day: Option<NaiveDate>,
) -> StoredWallet {
    StoredWallet {
        enabled: enabled != 0,
        secret_encrypted: secret.filter(|s| !s.is_empty()),
        max_boost_sats: max_boost,
        stream_sats_per_minute: stream,
        daily_limit_sats: daily,
        spent_day_sats: spent,
        spent_day,
    }
}

async fn read_settings(db: &DatabasePool, user_id: i32) -> AppResult<StoredWallet> {
    dialect_match!(db, {
        pg(pool) => {
            let row = q::query(
                r#"SELECT CASE WHEN enabled THEN 1 ELSE 0 END AS enabled,
                          nwcencrypted AS secret,
                          maxboostsats AS max_boost,
                          streampersats AS stream_sats,
                          dailylimitsats AS daily_limit,
                          spentdaysats AS spent,
                          spentday AS spent_day
                   FROM "ValueWallet" WHERE userid = $1"#,
            )
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
            Ok(match row {
                Some(row) => map_wallet(
                    row.try_get::<i32, _>("enabled")? as i64,
                    row.try_get("secret")?,
                    row.try_get("max_boost")?,
                    row.try_get("stream_sats")?,
                    row.try_get("daily_limit")?,
                    row.try_get("spent")?,
                    row.try_get("spent_day")?,
                ),
                None => disabled_wallet(),
            })
        },
        mysql(pool) => {
            let row = sqlx::query(
                r#"SELECT IF(Enabled, 1, 0) AS enabled,
                          NwcEncrypted AS secret,
                          MaxBoostSats AS max_boost,
                          StreamPerSats AS stream_sats,
                          DailyLimitSats AS daily_limit,
                          SpentDaySats AS spent,
                          SpentDay AS spent_day
                   FROM ValueWallet WHERE UserID = ?"#,
            )
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
            Ok(match row {
                Some(row) => {
                    let enabled: i64 = row
                        .try_get::<i64, _>("enabled")
                        .or_else(|_| row.try_get::<i32, _>("enabled").map(|v| v as i64))?;
                    let number = |name| -> Result<i64, sqlx::Error> {
                        row.try_get::<i64, _>(name)
                            .or_else(|_| row.try_get::<i32, _>(name).map(|v| v as i64))
                    };
                    map_wallet(
                        enabled,
                        row.try_get("secret")?,
                        number("max_boost")?,
                        number("stream_sats")?,
                        number("daily_limit")?,
                        number("spent")?,
                        row.try_get("spent_day")?,
                    )
                }
                None => disabled_wallet(),
            })
        },
    })
}

async fn write_settings(db: &DatabasePool, user_id: i32, stored: &StoredWallet) -> AppResult<()> {
    dialect_match!(db, {
        pg(pool) => {
            q::query(
                r#"INSERT INTO "ValueWallet"
                     (userid, enabled, nwcencrypted, maxboostsats, streampersats, dailylimitsats, spentdaysats, spentday, updatedat)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW())
                   ON CONFLICT (userid) DO UPDATE SET
                     enabled = EXCLUDED.enabled,
                     nwcencrypted = EXCLUDED.nwcencrypted,
                     maxboostsats = EXCLUDED.maxboostsats,
                     streampersats = EXCLUDED.streampersats,
                     dailylimitsats = EXCLUDED.dailylimitsats,
                     updatedat = NOW()"#,
            )
            .bind(user_id)
            .bind(stored.enabled)
            .bind(&stored.secret_encrypted)
            .bind(stored.max_boost_sats)
            .bind(stored.stream_sats_per_minute)
            .bind(stored.daily_limit_sats)
            .bind(stored.spent_day_sats)
            .bind(stored.spent_day)
            .execute(pool)
            .await?;
        },
        mysql(pool) => {
            sqlx::query(
                r#"INSERT INTO ValueWallet
                     (UserID, Enabled, NwcEncrypted, MaxBoostSats, StreamPerSats, DailyLimitSats, SpentDaySats, SpentDay, UpdatedAt)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, NOW())
                   ON DUPLICATE KEY UPDATE
                     Enabled = VALUES(Enabled),
                     NwcEncrypted = VALUES(NwcEncrypted),
                     MaxBoostSats = VALUES(MaxBoostSats),
                     StreamPerSats = VALUES(StreamPerSats),
                     DailyLimitSats = VALUES(DailyLimitSats),
                     UpdatedAt = NOW()"#,
            )
            .bind(user_id)
            .bind(stored.enabled)
            .bind(&stored.secret_encrypted)
            .bind(stored.max_boost_sats)
            .bind(stored.stream_sats_per_minute)
            .bind(stored.daily_limit_sats)
            .bind(stored.spent_day_sats)
            .bind(stored.spent_day)
            .execute(pool)
            .await?;
        },
    });
    Ok(())
}

async fn reserve_budget(db: &DatabasePool, user_id: i32, amount: i64) -> AppResult<bool> {
    if amount <= 0 {
        return Ok(false);
    }
    let changed = dialect_match!(db, {
        pg(pool) => {
            q::query(
                r#"UPDATE "ValueWallet"
                   SET spentdaysats = CASE
                         WHEN spentday IS DISTINCT FROM (CURRENT_TIMESTAMP AT TIME ZONE 'UTC')::date THEN $1
                         ELSE spentdaysats + $1
                       END,
                       spentday = (CURRENT_TIMESTAMP AT TIME ZONE 'UTC')::date,
                       updatedat = CURRENT_TIMESTAMP
                   WHERE userid = $2
                     AND enabled = TRUE
                     AND dailylimitsats > 0
                     AND (
                       CASE
                         WHEN spentday IS DISTINCT FROM (CURRENT_TIMESTAMP AT TIME ZONE 'UTC')::date THEN $1
                         ELSE spentdaysats + $1
                       END
                     ) <= dailylimitsats"#,
            )
            .bind(amount)
            .bind(user_id)
            .execute(pool)
            .await?
            .rows_affected()
        },
        mysql(pool) => {
            sqlx::query(
                r#"UPDATE ValueWallet
                   SET SpentDaySats = IF(SpentDay IS NULL OR SpentDay <> UTC_DATE(), ?, SpentDaySats + ?),
                       SpentDay = UTC_DATE(),
                       UpdatedAt = CURRENT_TIMESTAMP
                   WHERE UserID = ?
                     AND Enabled = TRUE
                     AND DailyLimitSats > 0
                     AND IF(SpentDay IS NULL OR SpentDay <> UTC_DATE(), ?, SpentDaySats + ?) <= DailyLimitSats"#,
            )
            .bind(amount)
            .bind(amount)
            .bind(user_id)
            .bind(amount)
            .bind(amount)
            .execute(pool)
            .await?
            .rows_affected()
        },
    });
    Ok(changed > 0)
}

async fn release_budget(db: &DatabasePool, user_id: i32, amount: i64) -> AppResult<()> {
    if amount <= 0 {
        return Ok(());
    }
    dialect_match!(db, {
        pg(pool) => {
            q::query(
                r#"UPDATE "ValueWallet"
                   SET spentdaysats = GREATEST(spentdaysats - $1, 0),
                       updatedat = CURRENT_TIMESTAMP
                   WHERE userid = $2
                     AND spentday = (CURRENT_TIMESTAMP AT TIME ZONE 'UTC')::date"#,
            )
            .bind(amount)
            .bind(user_id)
            .execute(pool)
            .await?;
        },
        mysql(pool) => {
            sqlx::query(
                r#"UPDATE ValueWallet
                   SET SpentDaySats = GREATEST(SpentDaySats - ?, 0),
                       UpdatedAt = CURRENT_TIMESTAMP
                   WHERE UserID = ?
                     AND SpentDay = UTC_DATE()"#,
            )
            .bind(amount)
            .bind(user_id)
            .execute(pool)
            .await?;
        },
    });
    Ok(())
}

async fn claim_minute(db: &DatabasePool, user_id: i32, episode_id: i32, minute: i32) -> AppResult<bool> {
    let changed = dialect_match!(db, {
        pg(pool) => {
            q::query(
                r#"INSERT INTO "ValueStreamMinute" (userid, episodeid, minute)
                   VALUES ($1, $2, $3)
                   ON CONFLICT DO NOTHING"#,
            )
            .bind(user_id)
            .bind(episode_id)
            .bind(minute)
            .execute(pool)
            .await?
            .rows_affected()
        },
        mysql(pool) => {
            sqlx::query(
                r#"INSERT IGNORE INTO ValueStreamMinute (UserID, EpisodeID, Minute)
                   VALUES (?, ?, ?)"#,
            )
            .bind(user_id)
            .bind(episode_id)
            .bind(minute)
            .execute(pool)
            .await?
            .rows_affected()
        },
    });
    Ok(changed > 0)
}

async fn release_minute(db: &DatabasePool, user_id: i32, episode_id: i32, minute: i32) -> AppResult<()> {
    dialect_match!(db, {
        pg(pool) => {
            q::query(
                r#"DELETE FROM "ValueStreamMinute" WHERE userid = $1 AND episodeid = $2 AND minute = $3"#,
            )
            .bind(user_id)
            .bind(episode_id)
            .bind(minute)
            .execute(pool)
            .await?;
        },
        mysql(pool) => {
            sqlx::query(
                r#"DELETE FROM ValueStreamMinute WHERE UserID = ? AND EpisodeID = ? AND Minute = ?"#,
            )
            .bind(user_id)
            .bind(episode_id)
            .bind(minute)
            .execute(pool)
            .await?;
        },
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn recipient(name: &str, kind: &str, address: &str, split: &str, fee: bool) -> ValueRecipient {
        ValueRecipient {
            name: name.to_string(),
            kind: kind.to_string(),
            address: address.to_string(),
            split: split.to_string(),
            custom_key: None,
            custom_value: None,
            fee: Some(fee),
        }
    }

    fn lightning_block(recipients: Vec<ValueRecipient>, time_splits: Vec<ValueTimeSplit>) -> ValueBlock {
        ValueBlock {
            kind: "lightning".to_string(),
            method: "keysend".to_string(),
            suggested: Some("1000".to_string()),
            recipients,
            time_splits,
        }
    }

    #[test]
    fn splits_fees_then_normalizes_the_rest() {
        let block = lightning_block(
            vec![
                recipient("host", "node", "02host", "90", false),
                recipient("app", "node", "02app", "10", true),
            ],
            vec![],
        );
        let allocation = allocate(100, &block, None, None);
        assert_eq!(allocation.shares.len(), 2);
        assert_eq!(allocation.shares[0].name, "app");
        assert_eq!(allocation.shares[0].sats, 10);
        assert!(allocation.shares[0].fee);
        assert_eq!(allocation.shares[1].name, "host");
        assert_eq!(allocation.shares[1].sats, 90);
        assert_eq!(allocation.unallocated_sats, 0);
    }

    #[test]
    fn skipped_split_is_not_paid_to_someone_else() {
        let block = lightning_block(
            vec![
                recipient("host", "node", "02host", "90", false),
                recipient("other", "hive", "abc", "10", false),
            ],
            vec![],
        );
        let allocation = allocate(100, &block, None, None);
        assert_eq!(allocation.shares.len(), 1);
        assert_eq!(allocation.shares[0].sats, 90);
        assert_eq!(allocation.unallocated_sats, 10);
        assert_eq!(allocation.skipped[0].kind, "hive");
    }

    #[test]
    fn fees_are_capped_at_the_total() {
        let block = lightning_block(
            vec![
                recipient("a", "node", "02a", "80", true),
                recipient("b", "node", "02b", "40", true),
            ],
            vec![],
        );
        let allocation = allocate(100, &block, None, None);
        let paid: u64 = allocation.shares.iter().map(|s| s.sats).sum();
        assert_eq!(paid, 100);
        assert!(allocation.shares.iter().all(|s| s.fee));
    }

    #[test]
    fn time_split_takes_its_percentage_and_empty_remote_stays_local() {
        let remote = ValueTimeSplit {
            start_time: "0".to_string(),
            duration: "30".to_string(),
            remote_percentage: Some("50".to_string()),
            recipients: vec![recipient("guest", "lnaddress", "guest@example.com", "100", false)],
            remote_item: None,
            ..ValueTimeSplit::default()
        };
        let block = lightning_block(
            vec![recipient("host", "node", "02host", "100", false)],
            vec![remote],
        );
        let inside = allocate(100, &block, Some(10), None);
        assert_eq!(inside.shares.iter().find(|s| s.name == "guest").unwrap().sats, 50);
        assert_eq!(inside.shares.iter().find(|s| s.name == "host").unwrap().sats, 50);

        let outside = allocate(100, &block, Some(40), None);
        assert_eq!(outside.shares.len(), 1);
        assert_eq!(outside.shares[0].name, "host");
        assert_eq!(outside.shares[0].sats, 100);

        let empty_remote = lightning_block(
            vec![recipient("host", "node", "02host", "100", false)],
            vec![ValueTimeSplit {
                start_time: "0".to_string(),
                duration: "30".to_string(),
                remote_item: Some(crate::services::podcasting2::RemoteItem {
                    feed_guid: "abc".to_string(),
                    ..Default::default()
                }),
                ..ValueTimeSplit::default()
            }],
        );
        let stayed = allocate(100, &empty_remote, Some(5), None);
        assert!(stayed.remote_feed_not_loaded);
        assert_eq!(stayed.shares[0].sats, 100);
        assert_eq!(stayed.shares[0].name, "host");
        let no_recipients: &[ValueRecipient] = &[];
        let not_found = allocate(100, &empty_remote, Some(5), Some(no_recipients));
        assert!(not_found.remote_feed_not_loaded);
        assert_eq!(not_found.shares[0].sats, 100);
        assert_eq!(not_found.shares[0].name, "host");
    }

    fn sats_of(allocation: &Allocation, name: &str) -> u64 {
        allocation
            .shares
            .iter()
            .find(|share| share.name == name)
            .unwrap_or_else(|| panic!("no share for {name}"))
            .sats
    }

    fn remote_split_block(inline: Vec<ValueRecipient>) -> ValueBlock {
        lightning_block(
            vec![
                recipient("host", "node", "02host", "90", false),
                recipient("app", "node", "02app", "10", true),
            ],
            vec![ValueTimeSplit {
                start_time: "0".to_string(),
                duration: "30".to_string(),
                remote_percentage: Some("50".to_string()),
                recipients: inline,
                remote_item: Some(RemoteItem {
                    feed_guid: "abc".to_string(),
                    ..Default::default()
                }),
                ..ValueTimeSplit::default()
            }],
        )
    }

    #[test]
    fn remote_item_split_pays_the_remote_block() {
        let block = remote_split_block(vec![]);
        let remote = vec![
            recipient("artist", "node", "02artist", "95", false),
            recipient("label", "node", "02label", "5", true),
        ];
        // 1000 sats: the local fee takes 100 and half of the other 900 goes to the remote block,
        // whose own fee takes 5% of those 450 (22, rounded down) before the artist gets 428.
        let inside = allocate(1000, &block, Some(5), Some(&remote));
        assert_eq!(sats_of(&inside, "app"), 100);
        assert_eq!(sats_of(&inside, "label"), 22);
        assert_eq!(sats_of(&inside, "artist"), 428);
        assert_eq!(sats_of(&inside, "host"), 450);
        assert_eq!(inside.shares.iter().map(|share| share.sats).sum::<u64>(), 1000);
        assert_eq!(inside.unallocated_sats, 0);
        assert!(!inside.remote_feed_not_loaded);

        let outside = allocate(1000, &block, Some(40), Some(&remote));
        assert_eq!(outside.shares.len(), 2);
        assert_eq!(sats_of(&outside, "app"), 100);
        assert_eq!(sats_of(&outside, "host"), 900);
    }

    #[test]
    fn unpayable_remote_recipients_are_skipped_and_their_split_is_withheld() {
        let block = remote_split_block(vec![]);
        let remote = vec![
            recipient("artist", "node", "02artist", "90", false),
            recipient("other", "hive", "abc", "10", false),
        ];
        let inside = allocate(1000, &block, Some(5), Some(&remote));
        assert_eq!(sats_of(&inside, "artist"), 405);
        assert_eq!(sats_of(&inside, "host"), 450);
        assert_eq!(inside.unallocated_sats, 45);
        assert_eq!(inside.skipped.len(), 1);
        assert_eq!(inside.skipped[0].name, "other");
    }

    #[test]
    fn a_split_with_its_own_recipients_ignores_the_remote_block() {
        let block = remote_split_block(vec![recipient("guest", "node", "02guest", "100", false)]);
        let remote = vec![recipient("artist", "node", "02artist", "100", false)];
        let inside = allocate(1000, &block, Some(5), Some(&remote));
        assert_eq!(sats_of(&inside, "guest"), 450);
        assert_eq!(sats_of(&inside, "host"), 450);
        assert!(inside.shares.iter().all(|share| share.name != "artist"));
    }

    const REMOTE_FEED: &str = r#"<rss version="2.0" xmlns:podcast="https://podcastindex.org/namespace/1.0"><channel>
<title>Remote</title>
<podcast:guid>abc</podcast:guid>
<podcast:value type="lightning" method="keysend">
  <podcast:valueRecipient name="chan" type="node" address="02chan" split="100"/>
</podcast:value>
<item><title>Track</title><guid>track-1</guid>
  <podcast:value type="lightning" method="keysend">
    <podcast:valueRecipient name="artist" type="node" address="02artist" split="100"/>
    <podcast:valueTimeSplit startTime="0" duration="10">
      <podcast:valueRecipient name="nested" type="node" address="02nested" split="100"/>
    </podcast:valueTimeSplit>
  </podcast:value>
</item>
</channel></rss>"#;

    fn names(recipients: Option<Vec<ValueRecipient>>) -> Option<Vec<String>> {
        recipients.map(|list| list.into_iter().map(|recipient| recipient.name).collect())
    }

    #[test]
    fn remote_recipients_come_from_the_item_block_then_the_channel_block() {
        let item = Some(vec!["artist".to_string()]);
        let channel = Some(vec!["chan".to_string()]);
        assert_eq!(names(recipients_from_feed(REMOTE_FEED, "abc", Some("track-1"))), item);
        assert_eq!(names(recipients_from_feed(REMOTE_FEED, " ABC ", Some(" track-1 "))), item);
        assert_eq!(names(recipients_from_feed(REMOTE_FEED, "abc", Some("missing"))), channel);
        assert_eq!(names(recipients_from_feed(REMOTE_FEED, "abc", None)), channel);
        assert_eq!(names(recipients_from_feed(REMOTE_FEED, "other", Some("track-1"))), None);
        // A feed that declares no guid cannot contradict the one that was asked for.
        let unnamed = REMOTE_FEED.replace("<podcast:guid>abc</podcast:guid>", "");
        assert_eq!(names(recipients_from_feed(&unnamed, "other", None)), channel);
        // No value block anywhere: nobody to pay.
        assert_eq!(recipients_from_feed("<rss><channel/></rss>", "abc", None), None);
    }

    #[test]
    fn a_search_answer_names_a_feed_url_only_for_the_wanted_guid() {
        let single = serde_json::json!({"feed": {"podcastGuid": "ABC", "url": "https://x/rss"}});
        assert_eq!(feed_url_from_search(&single, "abc").as_deref(), Some("https://x/rss"));
        let other = serde_json::json!({"feeds": [{"podcastGuid": "zzz", "url": "https://y"}]});
        assert_eq!(feed_url_from_search(&other, "abc"), None);
        let listed = serde_json::json!({"feeds": [
            {"podcastGuid": "zzz", "url": "https://y"},
            {"podcastGuid": "abc", "url": "http://z/rss"}
        ]});
        assert_eq!(feed_url_from_search(&listed, "abc").as_deref(), Some("http://z/rss"));
        let ftp = serde_json::json!({"feed": {"podcastGuid": "abc", "url": "ftp://x/rss"}});
        assert_eq!(feed_url_from_search(&ftp, "abc"), None);
        // Podcast Index answers an unknown GUID with an empty list in place of the feed.
        let missing = serde_json::json!({"status": "false", "feed": []});
        assert_eq!(feed_url_from_search(&missing, "abc"), None);
    }

    #[test]
    fn remote_value_cache_returns_hits_and_misses_and_stays_bounded() {
        let key = |n: usize| (1, format!("cache-test-{n}"), String::new(), String::new());
        store_remote_value(key(0), Some(vec![recipient("a", "node", "02a", "100", false)]));
        store_remote_value(key(1), None);
        let hit = cached_remote_value(&key(0)).flatten().unwrap();
        assert_eq!(hit[0].name, "a");
        assert_eq!(cached_remote_value(&key(1)), Some(None));
        assert_eq!(cached_remote_value(&key(2)), None);
        // Another user asking for the same guid, or the same guid with another feedUrl, is another entry.
        let mut other_user = key(0);
        other_user.0 = 2;
        assert_eq!(cached_remote_value(&other_user), None);
        let mut other_url = key(0);
        other_url.3 = "https://elsewhere.example/feed.xml".to_string();
        assert_eq!(cached_remote_value(&other_url), None);
        for n in 3..(3 + REMOTE_VALUE_MAX_ENTRIES + 2) {
            store_remote_value(key(n), None);
        }
        assert!(REMOTE_VALUE.lock().unwrap().len() <= REMOTE_VALUE_MAX_ENTRIES + 1);
    }

    #[test]
    fn episode_block_replaces_the_channel_block() {
        let episode = vec![lightning_block(vec![recipient("ep", "node", "02ep", "100", false)], vec![])];
        let channel = vec![lightning_block(vec![recipient("ch", "node", "02ch", "100", false)], vec![])];
        assert_eq!(select_block(&episode, &channel).unwrap().recipients[0].name, "ep");
        assert_eq!(select_block(&[], &channel).unwrap().recipients[0].name, "ch");
    }

    #[test]
    fn lightning_aliases_and_hex_tlv_values() {
        assert_eq!(
            classify_recipient(&recipient("n", "pubkey", "02aa", "100", false)),
            Some(PayKind::Keysend)
        );
        assert_eq!(
            classify_recipient(&recipient("n", "lightning", "a@b.co", "100", false)),
            Some(PayKind::LnAddress)
        );
        let pubkey = format!("02{}", "ab".repeat(32));
        assert_eq!(pubkey.len(), 66);
        assert_eq!(
            classify_recipient(&recipient("ace", "lnaddress", &pubkey, "5", false)),
            Some(PayKind::Keysend)
        );
        assert_eq!(
            classify_recipient(&recipient("ace", "lnaddress", "aceackerman@fountain.fm", "5", false)),
            Some(PayKind::LnAddress)
        );
        assert_eq!(classify_recipient(&recipient("n", "hive", "x", "100", false)), None);
        let mut btc = lightning_block(vec![], vec![]);
        btc.suggested = Some("0.00000005000".into());
        assert_eq!(suggested_sats(&btc), Some(5));
        btc.suggested = Some("0.00000001000".into());
        assert_eq!(suggested_sats(&btc), Some(1));
        btc.suggested = Some("1000".into());
        assert_eq!(suggested_sats(&btc), Some(1000));
        btc.suggested = Some("0".into());
        assert_eq!(suggested_sats(&btc), None);
        btc.suggested = Some("0.000000000".into());
        assert_eq!(suggested_sats(&btc), None);
        assert_eq!(tlv_hex("Ab"), "ab");
        assert_eq!(tlv_hex("hello"), hex::encode("hello"));
        assert_eq!(parse_split_hundredths("10.5"), 1_050);
    }

    #[test]
    fn boostagram_and_lnurl_urls_do_not_touch_the_network() {
        let meta = EpisodeMeta {
            podcast: "Show".to_string(),
            feed_id: "guid".to_string(),
            feed_url: "https://example.com/feed.xml".to_string(),
            episode: "Ep".to_string(),
            episode_guid: "ep".to_string(),
            podcast_id: 1,
            ts_seconds: 12,
            sender_name: "Ada".to_string(),
            message: "nice".to_string(),
        };
        let body = boostagram_json(&meta, "boost", "Host", 1_000, 2_000);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["action"], "boost");
        assert_eq!(parsed["name"], "Host");
        assert_eq!(parsed["sender_name"], "Ada");
        assert_eq!(parsed["value_msat"], 1000);
        assert!(lnurlp_url("not an address").is_err());
        assert_eq!(
            lnurlp_url("ada@example.com").unwrap(),
            "https://example.com/.well-known/lnurlp/ada"
        );
        let callback = callback_url("https://example.com/cb", 1_000, Some("hi")).unwrap();
        assert!(callback.contains("amount=1000"));
        assert!(callback.contains("comment=hi"));
        assert!(validate_nwc_uri("nostr+walletconnect://b889ff5b1513b641e2a139f661a661364979c5beee91842f8f0ef42ab558e9d4?secret=71a8c14c1407c113601079c4302dab36460f0ccd0ad506f1f2dc73b5100e4f3c&relay=wss%3A%2F%2Frelay.damus.io").is_ok());
        assert!(validate_nwc_uri("https://example.com").is_err());
    }

    #[test]
    fn budget_resets_on_a_new_utc_day_and_rejects_zero_limit() {
        let today = NaiveDate::from_ymd_opt(2026, 10, 22).unwrap();
        let yesterday = NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();
        assert_eq!(spent_after_reserve(true, 100, 90, Some(today), today, 20), None);
        assert_eq!(spent_after_reserve(true, 100, 90, Some(yesterday), today, 20), Some(20));
        assert_eq!(spent_after_reserve(true, 0, 0, None, today, 10), None);
        assert_eq!(spent_after_reserve(false, 100, 0, None, today, 10), None);
        assert_eq!(minute_index(125), 2);
        assert_eq!(minute_index(-5), 0);
    }

    struct MockWallet {
        fail: Mutex<Vec<String>>,
        keysends: Mutex<Vec<KeysendPayment>>,
        invoices: Mutex<Vec<(String, u64)>>,
    }

    #[async_trait]
    impl LightningWallet for MockWallet {
        async fn keysend(&self, payment: KeysendPayment) -> Result<String, String> {
            if self.fail.lock().unwrap().iter().any(|p| p == &payment.pubkey) {
                return Err("wallet rejected keysend".to_string());
            }
            let preimage = payment.preimage_hex.clone();
            self.keysends.lock().unwrap().push(payment);
            Ok(preimage)
        }
        async fn pay_invoice(&self, invoice: &str, amount_msat: u64) -> Result<String, String> {
            self.invoices.lock().unwrap().push((invoice.to_string(), amount_msat));
            Ok("invoice-preimage".to_string())
        }
        async fn balance_msat(&self) -> Result<u64, String> {
            Ok(50_000_000)
        }
    }

    struct MockHttp;

    #[async_trait]
    impl LnurlHttp for MockHttp {
        async fn get_text(&self, url: &str) -> Result<String, String> {
            if url.contains("/.well-known/") {
                return Ok(r#"{"callback":"https://example.com/cb","commentAllowed":20,"minSendable":1000,"maxSendable":100000000}"#.to_string());
            }
            assert!(url.contains("amount="));
            Ok(r#"{"pr":"lnbc1test"}"#.to_string())
        }
    }

    #[tokio::test]
    async fn mock_wallet_pays_keysend_tlv_and_lnaddress_without_a_network() {
        let shares = vec![
            Share {
                name: "Host".to_string(),
                address: "02host".to_string(),
                kind: PayKind::Keysend,
                sats: 90,
                custom_key: Some(696969),
                custom_value: Some("hello".to_string()),
                fee: false,
            },
            Share {
                name: "App".to_string(),
                address: "app@example.com".to_string(),
                kind: PayKind::LnAddress,
                sats: 10,
                custom_key: None,
                custom_value: None,
                fee: true,
            },
        ];
        let meta = EpisodeMeta {
            podcast: "Show".to_string(),
            feed_id: "guid".to_string(),
            feed_url: "https://example.com/feed.xml".to_string(),
            episode: "Ep".to_string(),
            episode_guid: "ep".to_string(),
            podcast_id: 1,
            ts_seconds: 3,
            sender_name: "Ada".to_string(),
            message: "nice episode".to_string(),
        };
        let wallet = MockWallet {
            fail: Mutex::new(vec![]),
            keysends: Mutex::new(vec![]),
            invoices: Mutex::new(vec![]),
        };
        let (paid, failed) = execute_payments(&shares, &meta, "boost", 100, &wallet, &MockHttp).await;
        assert!(failed.is_empty());
        assert_eq!(paid.len(), 2);
        let keysend = wallet.keysends.lock().unwrap();
        assert_eq!(keysend[0].amount_msat, 90_000);
        assert_eq!(keysend[0].tlv[0].0, KEYSEND_PREIMAGE_TLV);
        assert_eq!(keysend[0].tlv[0].1, keysend[0].preimage_hex);
        assert_eq!(keysend[0].tlv[1].0, BOOSTAGRAM_TLV);
        let decoded = String::from_utf8(hex::decode(&keysend[0].tlv[1].1).unwrap()).unwrap();
        assert!(decoded.contains("\"action\":\"boost\""));
        assert_eq!(keysend[0].tlv[2].0, 696969);
        assert_eq!(keysend[0].tlv[2].1, hex::encode("hello"));
        let invoices = wallet.invoices.lock().unwrap();
        assert_eq!(invoices[0].0, "lnbc1test");
        assert_eq!(invoices[0].1, 10_000);
    }

    #[tokio::test]
    async fn a_rejected_keysend_is_reported_and_does_not_stop_the_other_share() {
        let shares = vec![
            Share {
                name: "Bad".to_string(),
                address: "02bad".to_string(),
                kind: PayKind::Keysend,
                sats: 40,
                custom_key: None,
                custom_value: None,
                fee: false,
            },
            Share {
                name: "Good".to_string(),
                address: "02good".to_string(),
                kind: PayKind::Keysend,
                sats: 60,
                custom_key: None,
                custom_value: None,
                fee: false,
            },
        ];
        let meta = EpisodeMeta {
            podcast: "Show".to_string(),
            feed_id: String::new(),
            feed_url: String::new(),
            episode: "Ep".to_string(),
            episode_guid: String::new(),
            podcast_id: 1,
            ts_seconds: 0,
            sender_name: String::new(),
            message: String::new(),
        };
        let wallet = MockWallet {
            fail: Mutex::new(vec!["02bad".to_string()]),
            keysends: Mutex::new(vec![]),
            invoices: Mutex::new(vec![]),
        };
        let (paid, failed) = execute_payments(&shares, &meta, "stream", 100, &wallet, &MockHttp).await;
        assert_eq!(paid.len(), 1);
        assert_eq!(paid[0].name, "Good");
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].sats, 40);
    }
}
