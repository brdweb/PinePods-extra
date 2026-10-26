//! PinePods-extra features on episodes shared by a feed's subscribers: the second subscriber
//! sees their own podcast's episode guids and can plan the episode's media, even though the
//! episode row was stored under the first subscriber's podcast.

use pinepods_api::services::alternate_media::{plan_for_episode, Audience};

use crate::harness::*;

#[tokio::test(flavor = "multi_thread")]
async fn the_second_subscriber_gets_guids_and_a_media_plan_for_shared_episodes() {
    let app = TestApp::new().await;
    let (alice, bob) = (app.user("alice").await, app.user("bob").await);
    let url = app.feed("shared-extra.xml", &Feed::with_episodes("Show", 3));
    let a = app.subscribe(&alice, &url, 3).await;
    let b = app.subscribe(&bob, &url, 3).await;

    let alice_guids = app.db().podcast_episode_guids(a).await.unwrap();
    let bob_guids = app.db().podcast_episode_guids(b).await.unwrap();
    assert_eq!(alice_guids.len(), 3, "{alice_guids:?}");
    assert_eq!(bob_guids, alice_guids, "both podcasts list the feed's episodes");

    let episode = app.episode_id(&bob, b, "Episode 2").await;
    let plan = plan_for_episode(app.db(), episode, bob.id, None, Audience::Player)
        .await
        .expect("bob can plan the shared episode's media");
    assert!(!plan.playback.is_empty(), "{plan:?}");
    // A user who does not follow the feed still can't.
    let carol = app.user("carol").await;
    assert!(plan_for_episode(app.db(), episode, carol.id, None, Audience::Player).await.is_err());
}

/// A database migrated by an earlier PinePods-extra recorded its migrations as 063-065, the
/// numbers upstream later used. The runner relabels them 900-902 and then runs upstream's own
/// 063-065, so both sets end up applied under their own names.
#[tokio::test(flavor = "multi_thread")]
async fn fork_migrations_under_their_old_numbers_are_relabelled() {
    let app = TestApp::new().await;
    exec(&app, "DELETE FROM schema_migrations WHERE version IN ('063', '064', '065', '900', '901', '902')").await;
    for (version, name) in [("063", "podcasting2_full"), ("064", "music_assistant_settings"), ("065", "value_wallet")] {
        exec(
            &app,
            &format!("INSERT INTO schema_migrations (version, name, description, checksum) VALUES ('{version}', '{name}', 'fork', 'x')"),
        )
        .await;
    }
    rerun_migrations(&app, &["none"]).await;

    let name = |version: &'static str| {
        let app = &app;
        async move { text(app, &format!("SELECT name FROM schema_migrations WHERE version = '{version}'")).await }
    };
    assert_eq!(name("900").await.as_deref(), Some("podcasting2_full"));
    assert_eq!(name("901").await.as_deref(), Some("music_assistant_settings"));
    assert_eq!(name("902").await.as_deref(), Some("value_wallet"));
    assert_eq!(name("063").await.as_deref(), Some("add_feed_content_hash_and_refresh_not_before"));
    assert_eq!(name("064").await.as_deref(), Some("dedupe_episodes_by_guid"));
    assert_eq!(name("065").await.as_deref(), Some("unique_episode_guid_per_podcast"));
}
