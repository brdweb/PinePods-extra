//! Every header the client sets on a request must be one the bundled nginx allows
//! cross-origin. The desktop client's origin is `tauri://localhost`, so a header missing
//! from `Access-Control-Allow-Headers` gets the request blocked by CORS (upstream #1089).

use wasm_bindgen_test::wasm_bindgen_test;

const NGINX_CONF: &str = include_str!("../../../startup/nginx.conf");

/// Every file under `web/src` that calls `.header(`.
const SOURCES: &[(&str, &str)] = &[
    ("components/setting_components/auto_delete_settings.rs", include_str!("../components/setting_components/auto_delete_settings.rs")),
    ("components/setting_components/playback_settings.rs", include_str!("../components/setting_components/playback_settings.rs")),
    ("requests/discover_req.rs", include_str!("../requests/discover_req.rs")),
    ("requests/login_requests.rs", include_str!("../requests/login_requests.rs")),
    ("requests/music_assistant.rs", include_str!("../requests/music_assistant.rs")),
    ("requests/notify.rs", include_str!("../requests/notify.rs")),
    ("requests/people_req.rs", include_str!("../requests/people_req.rs")),
    ("requests/pod_req.rs", include_str!("../requests/pod_req.rs")),
    ("requests/search_pods.rs", include_str!("../requests/search_pods.rs")),
    ("requests/setting_reqs.rs", include_str!("../requests/setting_reqs.rs")),
    ("requests/stat_reqs.rs", include_str!("../requests/stat_reqs.rs")),
    ("requests/task_reqs.rs", include_str!("../requests/task_reqs.rs")),
    ("requests/value4value.rs", include_str!("../requests/value4value.rs")),
];

/// Lowercased header names from every `Access-Control-Allow-Headers` line in the nginx config.
fn allowed_headers(conf: &str) -> Vec<String> {
    const KEY: &str = "'Access-Control-Allow-Headers' '";
    conf.match_indices(KEY)
        .flat_map(|(i, _)| {
            let rest = &conf[i + KEY.len()..];
            let list = &rest[..rest.find('\'').unwrap_or(rest.len())];
            list.split(',').map(|h| h.trim().to_ascii_lowercase())
        })
        .filter(|h| !h.is_empty())
        .collect()
}

/// Header names passed as string literals to `.header(`.
fn header_literals(source: &str) -> Vec<&str> {
    source
        .match_indices(".header(")
        .filter_map(|(i, m)| {
            let rest = source[i + m.len()..].trim_start();
            let rest = rest.strip_prefix('"')?;
            Some(&rest[..rest.find('"')?])
        })
        .collect()
}

#[wasm_bindgen_test]
fn request_headers_are_allowed_by_nginx_cors() {
    let allowed = allowed_headers(NGINX_CONF);
    assert!(allowed.contains(&"api-key".to_string()), "nginx allow-list not found: {allowed:?}");
    let mut seen = 0;
    for (path, source) in SOURCES {
        for header in header_literals(source) {
            seen += 1;
            assert!(
                allowed.contains(&header.to_ascii_lowercase()),
                "web/src/{path} sends header {header:?}, which startup/nginx.conf does not allow \
                 cross-origin ({allowed:?}); the desktop client's request would be blocked"
            );
        }
    }
    assert!(seen > 100, "only {seen} header literals found; did the request code move?");
}

#[wasm_bindgen_test]
fn header_literals_reads_names_across_line_breaks() {
    let src = "Request::get(u)\n    .header(\"Api-Key\", &k)\n    .header(\n        \"X-Other\", v)\n    .header(name, v)";
    assert_eq!(header_literals(src), vec!["Api-Key", "X-Other"]);
}
