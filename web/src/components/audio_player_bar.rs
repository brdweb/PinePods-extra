use crate::components::audio::AudioPlayer;
use crate::components::context::UIState;
use crate::components::music_assistant::MaNowPlayingBar;
use crate::requests::music_assistant::{
    show_ma_now_playing, MusicAssistantState, PlaybackCandidate,
};
use crate::requests::now_playing::NowPlayingState;
use yew::{function_component, html, Html};
use yewdux::prelude::*;

/// Isolated AudioPlayer wrapper — owns the UIState subscription so that
/// audio state changes never cause parent page components to re-render.
/// When this device is idle (or paused) and a Music Assistant player has an
/// episode, the bar follows that player instead.
#[function_component(AudioPlayerBar)]
pub fn audio_player_bar() -> Html {
    let (audio_state, _) = use_store::<UIState>();
    let (ma, _) = use_store::<MusicAssistantState>();
    let (np, _) = use_store::<NowPlayingState>();
    let candidates: Vec<PlaybackCandidate> = np
        .ma_devices
        .iter()
        .map(|d| PlaybackCandidate {
            device_id: d.device_id.clone(),
            device_type: d.device_type.clone(),
            episode_id: d.episode_id,
            playing: d.playing,
            updated_at: d.updated_at,
        })
        .collect();
    let local_playing = audio_state.audio_playing == Some(true);
    if show_ma_now_playing(
        ma.enabled,
        local_playing,
        &candidates,
        ma.focus_device_id.as_deref(),
    ) {
        return html! { <MaNowPlayingBar /> };
    }
    if let Some(audio_props) = &audio_state.currently_playing {
        html! {
            <AudioPlayer
                episode={audio_props.episode.clone()}
                src={audio_props.src.clone()}
                title={audio_props.title.clone()}
                description={audio_props.description.clone()}
                release_date={audio_props.release_date.clone()}
                artwork_url={audio_props.artwork_url.clone()}
                duration={audio_props.duration.clone()}
                episode_id={audio_props.episode_id.clone()}
                duration_sec={audio_props.duration_sec.clone()}
                start_pos_sec={audio_props.start_pos_sec.clone()}
                end_pos_sec={audio_props.end_pos_sec.clone()}
                offline={audio_props.offline.clone()}
                is_youtube={audio_props.is_youtube.clone()}
                is_video={audio_props.is_video.clone()}
            />
        }
    } else {
        html! {}
    }
}
