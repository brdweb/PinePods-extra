//! File-name rules for locally downloaded episodes. Std-only so the tests run
//! without the Tauri toolchain.

/// Every extension `alternate_media::file_extension` in the server can return.
pub const EPISODE_EXTENSIONS: [&str; 9] = [
    "mp3", "m4a", "opus", "ogg", "flac", "wav", "webm", "mkv", "mp4",
];

/// The extension named by a `Content-Disposition: attachment; filename="..."`
/// header, or `mp3` when the header is missing or names an unknown type.
pub fn extension_from_disposition(content_disposition: Option<&str>) -> &'static str {
    let name = content_disposition
        .and_then(|header| header.split_once("filename="))
        .map(|(_, rest)| rest.trim_start_matches('"'))
        .map(|rest| rest.split(['"', ';']).next().unwrap_or(rest).trim());
    let ext = name
        .and_then(|name| name.rsplit_once('.'))
        .map(|(_, ext)| ext.to_ascii_lowercase());
    ext.and_then(|ext| EPISODE_EXTENSIONS.iter().copied().find(|known| *known == ext))
        .unwrap_or("mp3")
}

/// `requested` when it is exactly `episode_{id}.{ext}` for a known extension,
/// otherwise `episode_{id}.mp3`.
pub fn episode_filename(episode_id: i32, requested: Option<&str>) -> String {
    let accepted = requested.filter(|name| {
        EPISODE_EXTENSIONS
            .iter()
            .any(|ext| *name == format!("episode_{episode_id}.{ext}"))
    });
    match accepted {
        Some(name) => name.to_string(),
        None => format!("episode_{episode_id}.mp3"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_follows_the_server_file_name() {
        assert_eq!(
            extension_from_disposition(Some(
                "attachment; filename=\"2026-01-01_Show_-_Ep.m4a\""
            )),
            "m4a"
        );
        assert_eq!(
            extension_from_disposition(Some("attachment; filename=\"Show.OPUS\"")),
            "opus"
        );
    }

    #[test]
    fn unknown_or_missing_extensions_fall_back_to_mp3() {
        assert_eq!(extension_from_disposition(None), "mp3");
        assert_eq!(extension_from_disposition(Some("attachment")), "mp3");
        assert_eq!(
            extension_from_disposition(Some("attachment; filename=\"noext\"")),
            "mp3"
        );
        assert_eq!(
            extension_from_disposition(Some("attachment; filename=\"run.exe\"")),
            "mp3"
        );
    }

    #[test]
    fn only_episode_names_for_this_id_are_kept() {
        assert_eq!(
            episode_filename(7, Some("episode_7.opus")),
            "episode_7.opus"
        );
        assert_eq!(episode_filename(7, Some("../x.mp3")), "episode_7.mp3");
        assert_eq!(episode_filename(7, Some("episode_8.opus")), "episode_7.mp3");
        assert_eq!(episode_filename(7, None), "episode_7.mp3");
    }
}
