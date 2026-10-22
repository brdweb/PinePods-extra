use wasm_bindgen_test::*;
use crate::components::context::{AppState, PageLoadState};
use crate::pages::routes::Route;

wasm_bindgen_test_configure!(run_in_node_experimental);

#[wasm_bindgen_test]
fn test_app_compiles() {
    assert!(true);
}

#[wasm_bindgen_test]
fn test_basic_state() {
    let state = PageLoadState::default();
    assert!(state.is_loading.is_none());
}

#[wasm_bindgen_test]
fn test_with_output() {
    assert_eq!(2 + 2, 4);
}

#[wasm_bindgen_test]
fn test_route_variants_exist() {
    let routes = vec![
        Route::Home,
        Route::Login,
        Route::NotFound,
        Route::Settings,
        Route::Search,
        Route::Queue,
    ];
    assert!(!routes.is_empty());
}

#[wasm_bindgen_test]
fn test_route_variants() {
    let person_route = Route::Person {
        key: "12".to_string(),
    };
    let shared_ep_route = Route::SharedEpisode {
        url_key: "test_key".to_string(),
    };

    match person_route {
        Route::Person { key } => assert_eq!(key, "12"),
        _ => panic!("Wrong route type"),
    }

    match shared_ep_route {
        Route::SharedEpisode { url_key } => assert_eq!(url_key, "test_key"),
        _ => panic!("Wrong route type"),
    }
}

#[wasm_bindgen_test]
fn test_pkce_code_challenge_s256_rfc7636_vector() {
    // Test vector from RFC 7636 Appendix B.
    let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    let expected_challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
    assert_eq!(
        crate::requests::login_requests::pkce_code_challenge_s256(verifier),
        expected_challenge
    );
}

#[wasm_bindgen_test]
fn download_request_omits_unset_media_option() {
    let request = crate::requests::pod_req::DownloadEpisodeRequest {
        episode_id: 1,
        user_id: 2,
        is_youtube: false,
        media_option_id: None,
    };
    let value = serde_json::to_value(&request).unwrap();
    assert!(value.get("media_option_id").is_none());

    let request = crate::requests::pod_req::DownloadEpisodeRequest {
        episode_id: 1,
        user_id: 2,
        is_youtube: false,
        media_option_id: Some("alt-0".to_string()),
    };
    let value = serde_json::to_value(&request).unwrap();
    assert_eq!(value["media_option_id"], "alt-0");
}

#[wasm_bindgen_test]
fn reorder_playback_puts_selected_version_first() {
    use crate::requests::pod_req::PlaybackStep;
    let step = |id: &str, index: u32, uri: &str| PlaybackStep {
        option_id: id.to_string(),
        source_index: index,
        uri: uri.to_string(),
        mime_type: "audio/mpeg".to_string(),
        kind: "audio".to_string(),
        is_video: false,
    };
    let steps = vec![
        step("primary", 0, "https://cdn.example/a.mp3"),
        step("alt-0", 0, "https://mirror.example/a.mp3"),
        step("alt-0", 1, "https://backup.example/a.mp3"),
    ];
    let ordered = crate::components::media_options::reorder_playback(&steps, "alt-0");
    assert_eq!(ordered[0].uri, "https://mirror.example/a.mp3");
    assert_eq!(ordered[1].uri, "https://backup.example/a.mp3");
    assert_eq!(ordered[2].uri, "https://cdn.example/a.mp3");
}

#[wasm_bindgen_test]
fn test_pkce_code_challenge_s256_is_url_safe_unpadded() {
    let challenge = crate::requests::login_requests::pkce_code_challenge_s256("some-code-verifier");
    // base64url of a 32-byte digest: 43 chars, unpadded, no '+' or '/'
    assert_eq!(challenge.len(), 43);
    assert!(!challenge.contains('='));
    assert!(!challenge.contains('+'));
    assert!(!challenge.contains('/'));
}
