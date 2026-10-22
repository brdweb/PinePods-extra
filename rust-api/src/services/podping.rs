//! Optional Podping listener.
//!
//! Podping is a stream of Hive `custom_json` operations that name a feed which
//! just changed. When `PINEPODS_PODPING` is `1`, `true`, `on`, or `yes`, this
//! task polls a Hive API and refreshes subscribed feeds whose URL is in that
//! stream. Anything else, including an unset variable, leaves the listener off.
//!
//! Notifications are not fetched as feeds. A URL is refreshed only when it
//! matches a feed already stored for a subscription that still has refresh
//! turned on. The Hive API URL is checked with the public-URL guard before
//! every request. Tests use a scripted Hive client and do not open a socket.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde_json::{json, Value};
use tracing::{debug, info, warn};

use crate::handlers::refresh;
use crate::services::url_guard::{ensure_safe_public_url_async, guarded_redirect_policy};
use crate::AppState;
use std::sync::Arc;

const DEFAULT_API: &str = "https://api.hive.blog";
const POLL_SECONDS: u64 = 15;
const DEBOUNCE_SECONDS: u64 = 60;
const MAX_BLOCKS: u64 = 20;
const MAX_URLS_PER_OP: usize = 100;
const MAX_URL_LEN: usize = 2000;

pub fn parse_enabled(value: Option<&str>) -> bool {
    matches!(
        value.map(str::trim).map(|s| s.to_ascii_lowercase()).as_deref(),
        Some("1" | "true" | "on" | "yes")
    )
}

pub fn resolve_api_url(value: Option<&str>) -> String {
    value
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_API)
        .to_string()
}

/// Compare feed URLs without the differences Podping publishers tend to introduce:
/// host case, a trailing slash, and an explicit default port. The path and query
/// stay as published.
pub fn feed_key(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.len() > MAX_URL_LEN {
        return None;
    }
    let parsed = url::Url::parse(raw).ok()?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return None;
    }
    let host = parsed.host_str()?.to_ascii_lowercase();
    let port = match (parsed.scheme(), parsed.port()) {
        ("http", Some(80)) | ("https", Some(443)) | (_, None) => String::new(),
        (_, Some(port)) => format!(":{port}"),
    };
    let path = parsed.path().trim_end_matches('/');
    let query = parsed
        .query()
        .map(|query| format!("?{query}"))
        .unwrap_or_default();
    Some(format!("{}://{host}{port}{path}{query}", parsed.scheme()))
}

pub fn spawn(state: Arc<AppState>) {
    let configured = std::env::var("PINEPODS_PODPING").ok();
    if !parse_enabled(configured.as_deref()) {
        info!("Podping listener is off (set PINEPODS_PODPING=1 to refresh feeds from Hive)");
        return;
    }
    let api = resolve_api_url(std::env::var("PINEPODS_PODPING_API").ok().as_deref());
    let host = url::Url::parse(&api)
        .ok()
        .and_then(|parsed| parsed.host_str().map(str::to_string))
        .unwrap_or_else(|| "the configured Hive API".to_string());
    info!("Podping listener on ({host})");
    tokio::spawn(async move {
        run(state, api).await;
    });
}

async fn run(state: Arc<AppState>, api: String) {
    let client = match reqwest::Client::builder()
        .redirect(guarded_redirect_policy())
        .timeout(Duration::from_secs(15))
        .build()
    {
        Ok(client) => client,
        Err(err) => {
            warn!("Podping listener stopped: {err}");
            return;
        }
    };
    let rpc = HiveHttp { client, api };
    let mut cursor = None;
    let mut recent: HashMap<String, Instant> = HashMap::new();
    loop {
        match poll_since(&rpc, &mut cursor).await {
            Ok(urls) => {
                let pending = urls_to_refresh(&urls, &recent, Instant::now());
                if !pending.is_empty() {
                    match refresh::refresh_podping_urls(&state, &pending).await {
                        Ok(keys) if keys.is_empty() => {}
                        Ok(keys) => {
                            info!("Podping refreshed {} subscribed feed(s)", keys.len());
                            let now = Instant::now();
                            for key in keys {
                                recent.insert(key, now);
                            }
                        }
                        Err(err) => warn!("Podping refresh failed: {err}"),
                    }
                }
            }
            Err(err) => warn!("Podping poll failed: {err}"),
        }
        tokio::time::sleep(Duration::from_secs(POLL_SECONDS)).await;
    }
}

#[async_trait]
trait HiveRpc: Send + Sync {
    async fn call(&self, method: &str, params: &Value) -> Result<Value, String>;
}

struct HiveHttp {
    client: reqwest::Client,
    api: String,
}

#[async_trait]
impl HiveRpc for HiveHttp {
    async fn call(&self, method: &str, params: &Value) -> Result<Value, String> {
        ensure_safe_public_url_async(&self.api).await?;
        let response = self
            .client
            .post(&self.api)
            .json(&json!({
                "jsonrpc": "2.0",
                "method": method,
                "params": params,
                "id": 1
            }))
            .send()
            .await
            .map_err(|err| err.to_string())?;
        if !response.status().is_success() {
            return Err(format!("Hive API returned HTTP {}", response.status()));
        }
        let body: Value = response.json().await.map_err(|err| err.to_string())?;
        if let Some(err) = body.get("error") {
            return Err(format!("Hive API error: {err}"));
        }
        body.get("result")
            .cloned()
            .ok_or_else(|| "Hive API response has no result".to_string())
    }
}

/// Read new Podping URLs and advance `cursor` to the last block that was read.
/// The first call only records the head so a restart does not replay history.
async fn poll_since<R: HiveRpc>(
    rpc: &R,
    cursor: &mut Option<u64>,
) -> Result<Vec<String>, String> {
    let props = rpc
        .call("condenser_api.get_dynamic_global_properties", &json!([]))
        .await?;
    let head = props
        .get("head_block_number")
        .and_then(json_u64)
        .ok_or_else(|| "Hive head block is missing".to_string())?;
    let Some(previous) = *cursor else {
        *cursor = Some(head);
        debug!("Podping starting at Hive block {head}");
        return Ok(Vec::new());
    };
    if previous >= head {
        return Ok(Vec::new());
    }

    let mut from = previous.saturating_add(1);
    let behind = head.saturating_sub(from).saturating_add(1);
    if behind > MAX_BLOCKS {
        warn!(
            "Podping is {behind} blocks behind; reading the newest {MAX_BLOCKS} only"
        );
        from = head.saturating_sub(MAX_BLOCKS - 1);
    }
    let end = head.min(from.saturating_add(MAX_BLOCKS - 1));

    let mut urls = Vec::new();
    let mut next = previous;
    for block in from..=end {
        match rpc
            .call("condenser_api.get_ops_in_block", &json!([block, false]))
            .await
        {
            Ok(ops) => {
                urls.extend(urls_from_ops(&ops));
                next = block;
            }
            Err(err) => {
                *cursor = Some(next);
                if urls.is_empty() {
                    return Err(err);
                }
                warn!("Podping stopped at block {next}: {err}");
                return Ok(urls);
            }
        }
    }
    *cursor = Some(next);
    Ok(urls)
}

fn json_u64(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_i64().and_then(|n| u64::try_from(n).ok()))
        .or_else(|| value.as_f64().map(|n| n as u64))
}

pub(crate) fn urls_to_refresh(
    urls: &[String],
    recent: &HashMap<String, Instant>,
    now: Instant,
) -> Vec<String> {
    let window = Duration::from_secs(DEBOUNCE_SECONDS);
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for url in urls {
        let Some(key) = feed_key(url) else {
            continue;
        };
        if !seen.insert(key.clone()) {
            continue;
        }
        if recent
            .get(&key)
            .is_some_and(|seen_at| now.saturating_duration_since(*seen_at) < window)
        {
            continue;
        }
        out.push(url.clone());
    }
    out
}

fn urls_from_ops(ops: &Value) -> Vec<String> {
    let Some(rows) = ops.as_array() else {
        return Vec::new();
    };
    let mut urls = Vec::new();
    for row in rows {
        let Some(op) = row.get("op").and_then(Value::as_array) else {
            continue;
        };
        if op.first().and_then(Value::as_str) != Some("custom_json") {
            continue;
        }
        let Some(body) = op.get(1) else {
            continue;
        };
        let Some(id) = body.get("id").and_then(Value::as_str) else {
            continue;
        };
        if !is_podping_id(id) {
            continue;
        }
        match body.get("json") {
            Some(Value::String(text)) => push_payload_urls(&mut urls, text),
            Some(value) => push_payload_value(&mut urls, value),
            None => {}
        }
    }
    urls
}

fn is_podping_id(id: &str) -> bool {
    let id = id.trim();
    id.eq_ignore_ascii_case("podping") || id.to_ascii_lowercase().starts_with("pp_")
}

fn push_payload_urls(out: &mut Vec<String>, text: &str) {
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return;
    };
    push_payload_value(out, &value);
}

fn push_payload_value(out: &mut Vec<String>, value: &Value) {
    if let Some(urls) = value.get("urls") {
        collect_urls(out, urls);
    }
    if let Some(iris) = value.get("iris") {
        collect_urls(out, iris);
    }
}

fn collect_urls(out: &mut Vec<String>, value: &Value) {
    match value {
        Value::String(url) => push_http(out, url),
        Value::Array(items) => {
            for item in items {
                if out.len() >= MAX_URLS_PER_OP {
                    return;
                }
                if let Some(url) = item.as_str() {
                    push_http(out, url);
                } else if let Some(url) = item.get("url").and_then(Value::as_str) {
                    push_http(out, url);
                }
            }
        }
        Value::Object(_) => {
            if let Some(url) = value.get("url").and_then(Value::as_str) {
                push_http(out, url);
            }
        }
        _ => {}
    }
}

fn push_http(out: &mut Vec<String>, url: &str) {
    if out.len() >= MAX_URLS_PER_OP {
        return;
    }
    if feed_key(url).is_some() {
        out.push(url.trim().to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn op(id: &str, json_text: &str) -> Value {
        json!({
            "op": ["custom_json", {"id": id, "json": json_text}]
        })
    }

    #[test]
    fn the_listener_stays_off_unless_explicitly_enabled() {
        assert!(!parse_enabled(None));
        assert!(!parse_enabled(Some("")));
        assert!(!parse_enabled(Some("0")));
        assert!(!parse_enabled(Some("false")));
        assert!(!parse_enabled(Some("off")));
        assert!(parse_enabled(Some("1")));
        assert!(parse_enabled(Some("true")));
        assert!(parse_enabled(Some(" ON ")));
        assert!(parse_enabled(Some("yes")));
        assert_eq!(resolve_api_url(None), DEFAULT_API);
        assert_eq!(resolve_api_url(Some("  ")), DEFAULT_API);
        assert_eq!(
            resolve_api_url(Some("https://hive.example/")),
            "https://hive.example/"
        );
    }

    #[test]
    fn feed_keys_ignore_host_case_trailing_slash_and_default_port() {
        let left = feed_key("HTTPS://Example.COM:443/feed/").unwrap();
        let right = feed_key("https://example.com/feed").unwrap();
        assert_eq!(left, right);
        assert_eq!(left, "https://example.com/feed");
        assert_ne!(
            feed_key("https://example.com/feed?x=1").unwrap(),
            feed_key("https://example.com/feed").unwrap()
        );
        assert!(feed_key("ipfs://cid").is_none());
        assert!(feed_key("javascript:alert(1)").is_none());
    }

    #[test]
    fn legacy_and_iris_payloads_yield_http_urls_only() {
        let ops = json!([
            op("podping", r#"{"urls":["https://example.com/a","ipfs://cid"]}"#),
            op(
                "pp_podcast_update",
                r#"{"iris":[{"url":"https://example.com/b"},"https://example.com/c/"]}"#
            ),
            op("notify", r#"{"urls":["https://example.com/nope"]}"#),
            {"op": ["transfer", {"amount": "1.000 HIVE"}]},
            op("PP_Podcast_Live", r#"{"urls":"https://example.com/live"}"#)
        ]);
        let urls = urls_from_ops(&ops);
        assert_eq!(
            urls,
            vec![
                "https://example.com/a",
                "https://example.com/b",
                "https://example.com/c/",
                "https://example.com/live"
            ]
        );
    }

    #[test]
    fn a_recent_feed_is_not_refreshed_again() {
        let mut recent = HashMap::new();
        let now = Instant::now();
        recent.insert("https://example.com/a".to_string(), now);
        let urls = vec![
            "https://example.com/a/".to_string(),
            "https://example.com/b".to_string(),
            "https://example.com/b".to_string(),
        ];
        assert_eq!(
            urls_to_refresh(&urls, &recent, now),
            vec!["https://example.com/b".to_string()]
        );
    }

    struct ScriptedHive {
        head: u64,
        blocks: HashMap<u64, Value>,
        requested: std::sync::Mutex<Vec<u64>>,
    }

    #[async_trait]
    impl HiveRpc for ScriptedHive {
        async fn call(&self, method: &str, params: &Value) -> Result<Value, String> {
            match method {
                "condenser_api.get_dynamic_global_properties" => {
                    Ok(json!({"head_block_number": self.head}))
                }
                "condenser_api.get_ops_in_block" => {
                    let block = params.get(0).and_then(json_u64).unwrap_or(0);
                    self.requested.lock().unwrap().push(block);
                    Ok(self.blocks.get(&block).cloned().unwrap_or_else(|| json!([])))
                }
                other => Err(format!("unexpected method {other}")),
            }
        }
    }

    #[tokio::test]
    async fn the_first_poll_records_the_head_and_the_next_reads_only_new_blocks() {
        let hive = ScriptedHive {
            head: 50,
            blocks: HashMap::from([(
                51,
                json!([op("pp_podcast_update", r#"{"urls":["https://feeds.example/show"]}"#)]),
            )]),
            requested: std::sync::Mutex::new(Vec::new()),
        };
        let mut cursor = None;
        assert!(poll_since(&hive, &mut cursor).await.unwrap().is_empty());
        assert_eq!(cursor, Some(50));
        assert!(hive.requested.lock().unwrap().is_empty());

        let hive = ScriptedHive {
            head: 51,
            blocks: hive.blocks,
            requested: hive.requested,
        };
        let urls = poll_since(&hive, &mut cursor).await.unwrap();
        assert_eq!(urls, vec!["https://feeds.example/show".to_string()]);
        assert_eq!(cursor, Some(51));
        assert_eq!(*hive.requested.lock().unwrap(), vec![51]);
    }

    #[tokio::test]
    async fn a_long_gap_reads_the_newest_blocks_only() {
        let hive = ScriptedHive {
            head: 1000,
            blocks: HashMap::from([(
                1000,
                json!([op("podping", r#"{"urls":["https://feeds.example/new"]}"#)]),
            )]),
            requested: std::sync::Mutex::new(Vec::new()),
        };
        let mut cursor = Some(1);
        let urls = poll_since(&hive, &mut cursor).await.unwrap();
        assert_eq!(urls, vec!["https://feeds.example/new".to_string()]);
        assert_eq!(cursor, Some(1000));
        let requested = hive.requested.lock().unwrap().clone();
        assert_eq!(requested.first().copied(), Some(981));
        assert_eq!(requested.last().copied(), Some(1000));
        assert_eq!(requested.len(), 20);
    }
}
