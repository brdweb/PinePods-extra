//! Podcasting 2.0 tags: chapters, transcripts and people, however the feed is laid out.

use crate::harness::*;
use crate::harness::rss;

const CHAPTERS_JSON: &str = r#"{"version":"1.2.0","chapters":[{"startTime":0,"title":"Intro"},{"startTime":95,"title":"Main topic"}]}"#;

/// Item-level tags for one episode; `chapters` is the chapters JSON URL.
fn item_tags(chapters: &str) -> String {
    format!(
        "      <podcast:transcript url=\"https://cdn.example.invalid/t/ep.vtt\" type=\"text/vtt\" language=\"en\" rel=\"captions\"/>\n\
         \x20     <podcast:transcript url=\"https://cdn.example.invalid/t/ep.json\" type=\"application/json\"/>\n\
         \x20     <podcast:chapters url=\"{}\" type=\"application/json+chapters\"/>\n\
         \x20     <podcast:person role=\"guest\" img=\"https://example.invalid/g.jpg\" href=\"https://example.invalid/g\">Ana &amp; Bo</podcast:person>\n",
        rss::esc(chapters)
    )
}

const CHANNEL_PERSON: &str = "    <podcast:person role=\"host\">Hosty McHost</podcast:person>\n";

async fn p20(app: &TestApp, u: &TestUser, ep: i32) -> Value {
    app.get("/api/data/fetch_podcasting_2_data")
        .key(u)
        .query("episode_id", ep)
        .query("user_id", u.id)
        .send()
        .await
        .ok_json()
}

fn assert_full(data: &Value) {
    let chapter_titles: Vec<&str> =
        data["chapters"].as_array().unwrap().iter().map(|c| c["title"].as_str().unwrap()).collect();
    assert_eq!(chapter_titles, ["Intro", "Main topic"], "{data:#}");

    let transcripts = data["transcripts"].as_array().unwrap();
    assert_eq!(transcripts.len(), 2, "{data:#}");
    assert_eq!(transcripts[0]["url"], "https://cdn.example.invalid/t/ep.vtt");
    assert_eq!(transcripts[0]["mime_type"], "text/vtt");
    assert_eq!(transcripts[0]["language"], "en");
    assert_eq!(transcripts[0]["rel"], "captions");
    assert_eq!(transcripts[1]["mime_type"], "application/json");
    assert!(transcripts[1]["language"].is_null());

    let mut people: Vec<(String, String)> = data["people"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| (p["name"].as_str().unwrap().to_string(), p["role"].as_str().unwrap_or_default().to_string()))
        .collect();
    people.sort();
    assert_eq!(
        people,
        [("Ana & Bo".to_string(), "guest".to_string()), ("Hosty McHost".to_string(), "host".to_string())],
        "channel people plus the episode's own, entities decoded: {data:#}"
    );
}

/// Subscribe to `xml` and return (user, id of "Episode 2", id of "Episode 1").
async fn subscribe_raw(app: &TestApp, name: &str, xml: String) -> (TestUser, i32, i32) {
    let user = app.user(name).await;
    let url = app.fixtures().serve_feed(&format!("{name}.xml"), xml);
    let pod = app.subscribe(&user, &url, 2).await;
    let two = app.episode_id(&user, pod, "Episode 2").await;
    let one = app.episode_id(&user, pod, "Episode 1").await;
    (user, two, one)
}

#[tokio::test(flavor = "multi_thread")]
async fn chapters_transcripts_and_people_are_read_for_the_right_episode() {
    let app = TestApp::new().await;
    let chapters = app.fixtures().serve("chapters.json", Fixture::new("application/json", CHAPTERS_JSON));
    let feed = Feed::new("P20 Show")
        .extra(CHANNEL_PERSON)
        .episode(Episode::new(1))
        .episode(Episode::new(2).extra(&item_tags(&chapters)));
    let (user, two, one) = subscribe_raw(&app, "listener", feed.to_xml()).await;

    assert_full(&p20(&app, &user, two).await);

    // Episode 1 has no tags of its own: only the channel-level host shows up.
    let data = p20(&app, &user, one).await;
    assert_eq!(data["chapters"], json!([]), "{data:#}");
    assert_eq!(data["transcripts"], json!([]));
    let names: Vec<&str> = data["people"].as_array().unwrap().iter().map(|p| p["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Hosty McHost"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn tags_are_found_in_minified_feeds_and_before_the_enclosure() {
    let app = TestApp::new().await;
    let chapters = app.fixtures().serve("chapters.json", Fixture::new("application/json", CHAPTERS_JSON));

    // Tags ahead of <enclosure>, which many publishers emit.
    let feed = Feed::new("Early Tags").extra(CHANNEL_PERSON).episode(Episode::new(1)).episode(Episode::new(2));
    let early = feed.to_xml().replacen("<item>\n", &format!("<item>\n{}", item_tags(&chapters)), 1);
    let (user, two, _) = subscribe_raw(&app, "early", early.clone()).await;
    assert_full(&p20(&app, &user, two).await);

    // The same feed with every line break removed.
    let minified: String = early.lines().map(str::trim).collect();
    let (user, two, _) = subscribe_raw(&app, "minified", minified).await;
    assert_full(&p20(&app, &user, two).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn full_namespace_is_stored_and_served() {
    let app = TestApp::new().await;
    let chapters = app.fixtures().serve("chapters.json", Fixture::new("application/json", CHAPTERS_JSON));
    let xml = include_str!("../fixtures/podcasting2-namespace.xml")
        .replace("https://cdn.example.invalid/chapters.json", &chapters);
    let user = app.user("namespace").await;
    let url = app.fixtures().serve_feed("namespace.xml", xml);
    let pod = app.subscribe(&user, &url, 2).await;

    let tagged = app.episode_id(&user, pod, "Tagged Episode").await;
    let data = p20(&app, &user, tagged).await;
    assert_eq!(data["chapters"][0]["title"], "Intro", "{data:#}");
    assert_eq!(data["transcripts"][0]["rel"], "captions");
    let mut names: Vec<&str> = data["people"].as_array().unwrap().iter().map(|p| p["name"].as_str().unwrap()).collect();
    names.sort();
    assert_eq!(names, ["Ada & Host", "Gus Guest"], "{data:#}");
    let tags = &data["tags"];
    assert_eq!(tags["episode"]["number"], "12", "itunes:episode is not the Podcasting 2.0 episode: {tags:#}");
    assert_eq!(tags["episode"]["display"], "Ch.3");
    assert_eq!(tags["season"]["number"], "4");
    assert_eq!(tags["season"]["name"], "The Pilot");
    assert_eq!(tags["soundbites"][1]["title"], "Rock & Roll");
    assert_eq!(tags["alternate_enclosures"][0]["sources"][1]["uri"], "ipfs://bafybeigdyr");
    assert_eq!(tags["alternate_enclosures"][0]["integrity"]["type"], "sri");
    assert_eq!(tags["value"][0]["time_splits"][0]["remote_start_time"], "10");
    assert_eq!(tags["value"][0]["time_splits"][0]["recipients"][0]["name"], "Guest Split");
    assert_eq!(tags["remote_items"][0]["feed_guid"], "b40ffcf7-2c48-5cfe-8daa-b65d766b2c25");
    assert_eq!(tags["chat"]["protocol"], "matrix");
    assert_eq!(tags["blocks"][0]["id"], "amazon");

    let plain = app.episode_id(&user, pod, "Plain Episode").await;
    let plain_data = p20(&app, &user, plain).await;
    assert!(plain_data["tags"]["episode"].is_null(), "itunes:episode must not be stored: {plain_data:#}");
    let plain_names: Vec<&str> = plain_data["people"].as_array().unwrap().iter().map(|p| p["name"].as_str().unwrap()).collect();
    assert_eq!(plain_names, ["Ada & Host"]);

    let show = app
        .get("/api/data/fetch_podcasting_2_pod_data")
        .key(&user)
        .query("podcast_id", pod)
        .query("user_id", user.id)
        .send()
        .await
        .ok_json();
    assert_eq!(show["guid"], "917393e3-1b1e-5cef-ace4-edaa54e1f810");
    assert_eq!(show["podroll"][0]["feed_guid"], "29cdca4a-32d8-56ba-b48b-09a011c5daa9");
    assert_eq!(show["value"][0]["recipients"][0]["custom_key"], "696969");
    assert_eq!(show["tags"]["medium"], "music");
    assert_eq!(show["tags"]["locked"]["value"], "yes");
    assert_eq!(show["tags"]["update_frequency"]["rrule"], "FREQ=WEEKLY;INTERVAL=2");
    assert_eq!(show["tags"]["podping"]["uses_podping"], true);
    assert_eq!(show["tags"]["live_items"][0]["status"], "live");
    assert_eq!(show["tags"]["live_items"][0]["tags"]["people"][0]["name"], "Live Host");
    assert_eq!(show["tags"]["publisher"]["medium"], "publisher");
    assert_eq!(show["tags"]["complete"], true);
    assert_eq!(
        show["tags"]["chat"]["embed_url"],
        "https://kiwiirc.example.invalid/nextclient/"
    );
    assert_eq!(show["tags"]["live_items"][0]["live_value"]["protocol"], "socket.io");
    assert_eq!(
        show["tags"]["live_items"][0]["live_value"]["uri"],
        "https://live.example.invalid/socket.io/"
    );
    assert_eq!(
        show["tags"]["live_items"][0]["live_value"]["value"][0]["recipients"][0]["name"],
        "Live Split"
    );

    assert_eq!(refresh_interval(&app, pod).await, Some(14 * 24 * 60));
}

/// The media-options query reads `is_video` (Postgres) / `IsVideo` (MySQL). A wrong
/// column name 500s this endpoint and makes Music Assistant fall back to a stream URL.
#[tokio::test(flavor = "multi_thread")]
async fn episode_media_options_reads_is_video_and_alternate_enclosures() {
    use pinepods_api::services::alternate_media::{plan_for_episode, Audience};

    let app = TestApp::new().await;
    let alt = r#"<podcast:alternateEnclosure type="audio/mpeg" default="true" title="Mirror">
        <podcast:source uri="https://mirror.example.invalid/ep.mp3"/>
      </podcast:alternateEnclosure>"#;
    let feed = Feed::new("Media Options")
        .episode(Episode::new(1).extra(alt))
        .episode(
            Episode::new(2)
                .enclosure("https://cdn.example.invalid/video/ep2.mp4")
                .video(),
        );
    let (user, video_id, audio_id) = subscribe_raw(&app, "mediaopt", feed.to_xml()).await;

    let options = app
        .get("/api/data/episode_media_options")
        .key(&user)
        .query("episode_id", audio_id)
        .query("user_id", user.id)
        .send()
        .await
        .ok_json();
    assert_eq!(options["playback"][0]["uri"], "https://mirror.example.invalid/ep.mp3", "{options:#}");
    assert_eq!(options["playback"][0]["is_video"], false);

    let video = app
        .get("/api/data/episode_media_options")
        .key(&user)
        .query("episode_id", video_id)
        .query("user_id", user.id)
        .send()
        .await
        .ok_json();
    assert_eq!(video["playback"][0]["uri"], "https://cdn.example.invalid/video/ep2.mp4", "{video:#}");
    assert_eq!(video["playback"][0]["is_video"], true);

    let plan = plan_for_episode(app.db(), video_id, user.id, None, Audience::MusicAssistant)
        .await
        .expect("plan");
    assert_eq!(plan.playback[0].uri, "https://cdn.example.invalid/video/ep2.mp4");
    assert!(plan.playback[0].is_video);
}

async fn refresh_interval(app: &TestApp, podcast_id: i32) -> Option<i32> {
    use pinepods_api::database::DatabasePool;
    match app.db() {
        DatabasePool::Postgres(pool) => {
            sqlx::query_scalar(r#"SELECT p20refreshintervalminutes FROM "Podcasts" WHERE podcastid = $1"#)
                .bind(podcast_id)
                .fetch_one(pool)
                .await
                .unwrap()
        }
        DatabasePool::MySQL(pool) => sqlx::query_scalar("SELECT P20RefreshIntervalMinutes FROM Podcasts WHERE PodcastID = ?")
            .bind(podcast_id)
            .fetch_one(pool)
            .await
            .unwrap(),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn episodes_whose_audio_url_has_a_query_string_still_match() {
    let app = TestApp::new().await;
    let chapters = app.fixtures().serve("chapters.json", Fixture::new("application/json", CHAPTERS_JSON));
    let feed = Feed::new("Tracked")
        .extra(CHANNEL_PERSON)
        .episode(Episode::new(1))
        .episode(
            Episode::new(2)
                .enclosure("https://track.example.invalid/redirect?u=ep2.mp3&src=rss")
                .extra(&item_tags(&format!("{chapters}?v=1&fmt=json"))),
        );
    let (user, two, _) = subscribe_raw(&app, "tracked", feed.to_xml()).await;
    assert_full(&p20(&app, &user, two).await);
}
