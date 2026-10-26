# Music Assistant output

PinePods can play episodes on [Music Assistant](https://www.music-assistant.io/) (MA) players and groups. The bridge is opt-in, per user, and off until that user saves an enabled configuration. Web and mobile remote-device screens target other PinePods clients on the now-playing relay. Music Assistant players are not in that list. They appear only in the MA-Speakers panel. This document is the contract for the extra settings, volume, and grouping UI.

Music Assistant 2.10.x is the tested server. The live transport is MA's WebSocket (`ws://host:8095/ws`). MA also accepts JSON-RPC `POST /api` with `Authorization: Bearer <token>`; PinePods does not call that path.

## Settings

All routes live under `/api/data` and require an `Api-Key` header (`Api-Key`, `api-key`, or `X-API-Key`) that belongs to `user_id`. A missing or invalid key is 401. A key for a different user is 403. Error bodies are `{ "error", "message", "status_code" }`.

The long-lived MA token is stored with the same Fernet encryption as other secrets. Responses never include it; they include `has_token`.

No row in `MusicAssistant` (migration `064`) means the feature is off. Postgres columns are lowercase (`userid`, `enabled`, `serverurl`, `tokenencrypted`, `externalbaseurl`). MySQL columns are `UserID`, `Enabled`, `ServerUrl`, `TokenEncrypted`, `ExternalBaseUrl`.

### `GET /api/data/music_assistant/settings?user_id=`

Returns the saved configuration and the live session, if any.

```json
{
  "enabled": false,
  "server_url": null,
  "external_base_url": null,
  "has_token": false,
  "connected": false,
  "server_version": null,
  "schema_version": null,
  "warning": null,
  "last_error": null
}
```

An unset user returns `enabled: false` with null URLs. `connected` is true only while this API process holds a live MA session.

### `PUT /api/data/music_assistant/settings`

```json
{
  "user_id": 1,
  "enabled": true,
  "server_url": "http://192.168.1.20:8095",
  "external_base_url": "http://192.168.1.10:8040",
  "token": "long-lived-token",
  "clear_token": false
}
```

- `enabled` defaults to `false` when omitted.
- Omit `token`, or send `""`, to keep the stored token. `clear_token: true` deletes it.
- Omit a URL to keep the stored one. Sent URLs are normalized before they are stored.
- Enabling requires a server URL, an external base URL, and a token (new or already stored). Otherwise the call is 400.
- A successful save asks the supervisor to reconnect. The response is the same shape as GET. `connected` may still be false until the handshake finishes; poll GET or the players route.

`server_url` accepts `http`, `https`, `ws`, and `wss`. A missing port becomes `8095` for plain HTTP and `443` for HTTPS. The path may only be empty, `/`, `/ws`, or `/api`. Credentials are rejected. Loopback and private LAN addresses are allowed (Music Assistant usually sits on the LAN). Link-local addresses (`169.254.0.0/16`, `fe80::/10`) and `metadata.google.internal` are rejected. The stored value is the origin, for example `http://192.168.1.20:8095`.

`external_base_url` is the origin Music Assistant uses to fetch PinePods. It must be `http` or `https`, with no path, query, fragment, or credentials. `localhost`, `*.localhost`, loopback, unspecified (`0.0.0.0`, `::`), and link-local hosts are rejected. Private LAN addresses are allowed. Music Assistant cannot play a stream that only exists on PinePods' loopback.

### `POST /api/data/music_assistant/test`

Probes a server without saving. `server_url` and `token` override the stored values for this call only. Optional `external_base_url` is validated with the same rules as a saved external base URL and is not stored. A missing URL or token returns HTTP 200 with `ok: false` (the user is not configured yet). An illegal server URL or external base URL is 400 (including localhost, loopback, and link-local external base URLs). The probe does not open the stream from the Music Assistant host; clients should still tell the user that playback fails immediately when that host cannot fetch `external_base_url`.

```json
{
  "ok": true,
  "detail": "connected",
  "server_version": "2.10.3",
  "schema_version": 80,
  "player_count": 2,
  "warning": null
}
```

`ok` is false when authentication fails or the server is older than 2.6. A warning (older than 2.10, newer than 2.x, unparsable version, low schema) still has `ok: true`.

### `GET /api/data/music_assistant/players?user_id=`

```json
{
  "enabled": true,
  "connected": true,
  "players": [
    {
      "player_id": "kitchen",
      "device_id": "ma:1:kitchen",
      "name": "Kitchen",
      "kind": "player",
      "available": true,
      "state": "idle",
      "volume_level": 20,
      "volume_muted": false,
      "group_members": [],
      "synced_to": null,
      "can_group_with": ["living"],
      "queue_id": "kitchen"
    }
  ]
}
```

`kind` is `player` or `group` (MA `type` containing `group`). Sync leaders stay `player` and still list `group_members`. `queue_id` is `active_source` when the player is grouped, otherwise `player_id`. Unavailable players stay in this list. The call returns an empty `players` array when the feature is off or the session is not up yet; it does not 500.

### `POST /api/data/music_assistant/command`

```json
{
  "user_id": 1,
  "player_id": "kitchen",
  "action": "play_episode",
  "args": { "episode_id": 42, "is_youtube": false }
}
```

`player_id` may be the raw MA id or the relay device id `ma:{user_id}:{player_id}`. The same applies to `target_player` and to ids inside `player_ids`, `members`, `player_ids_to_add`, and `player_ids_to_remove`. Player ids can contain colons (MAC addresses); strip only the `ma:{user_id}:` prefix.

Success:

```json
{ "ok": true, "detail": "ok", "warnings": [] }
```

| Status | When |
| --- | --- |
| 400 | Unknown action, missing arguments, MA rejected the command, the episode does not exist, or `next` has no PinePods episode loaded |
| 503 | This process has no session for the user |

The first MA command must succeed. Follow-up seek and speed changes after `play_episode` are best-effort and show up in `warnings`.

A `next` with nothing up next succeeds with `"detail": "nothing_next"` and sends no command; the speaker keeps playing. A `next` on a speaker with no PinePods episode loaded is a 400.

## Playback

`play_episode` loads the episode, then sends MA `player_queues/play_media` with `option: "replace"`.

When the episode is already downloaded on PinePods, or it is a YouTube video, the URL is the PinePods stream:

```
{external_base_url}/api/data/stream/{episode_id}?api_key={scoped_rss_key}&user_id={user_id}
```

YouTube episodes add `&type=youtube`. The key is a podcast-scoped RSS key for that episode's podcast (created if one does not already exist). It is not the user's full API key.

When the episode is not downloaded, Music Assistant is given a playable `http`/`https` file directly. The choice prefers a progressive audio file over a default HLS stream, and it never sends IPFS or a torrent. Optional `args.media_option_id` (`primary` or `alt-N`, the same ids as `GET /api/data/episode_media_options`) selects that version when it is a file a speaker can fetch. Feed username and password are not added to the direct URL. Download the episode in PinePods first when the host requires those credentials; Music Assistant then streams the saved file.

Optional `args.playlist_id` is the playlist the episode was started from. The bridge keeps it with the episode, so `next` plays the next item of that playlist.

If `args.position_sec` is present, including `0`, playback seeks there. Otherwise the stored listen position is used. A position of at least 1 second is sent as `player_queues/seek` after `play_media`. Playback speed is not sent. Episodes are `media_type: track`, and Music Assistant only tempo-shifts podcast episodes and audiobooks, so a local speed other than 1x stays on the browser. An incoming `set_speed` is ignored and is not shown as an error.

## Now-playing relay

Each **available** player and group is registered as a virtual device on the existing socket:

`/ws/api/nowplaying/{user_id}?api_key=…&device_id=…&device_name=…`

| Field | Value |
| --- | --- |
| `device_id` | `ma:{user_id}:{player_id}` |
| `device_type` | `music_assistant` |
| `device_name` | MA display name |
| `capabilities` | listed below |

Unavailable players are listed by the HTTP players route and are not registered as devices. A transient MA disconnect keeps the registration so in-flight commands can queue; devices disappear when the user disables the bridge or changes the server.

Existing clients can already send `play`, `pause`, `stop`, `seek`, `skip_forward`, `skip_back`, and `play_episode`. Music Assistant devices also advertise `next` and `sleep_timer`. Send them the same way as for any other device. `set_speed` is accepted and ignored; it is not in the advertised capabilities.

```json
{
  "type": "command",
  "target_device_id": "ma:1:kitchen",
  "action": "play_episode",
  "args": { "episode_id": 42, "is_youtube": false, "position_sec": 90 },
  "cmd_id": "optional"
}
```

The ack `{"type":"ack","ok":true,"detail":"relayed"}` means the virtual device accepted the frame. It does not mean MA finished the command. `ok: false` means the device is not registered.

### New actions

Other device types ignore these. Only send them when `capabilities` contains the name.

| Action | Args | MA command |
| --- | --- | --- |
| `volume_set` | `volume_level` or `volume` (0–100, clamped) | `players/cmd/volume_set` |
| `volume_mute` | `muted` or `mute` (bool) | `players/cmd/volume_mute` |
| `group` | `target_player` (required) | `players/cmd/group` |
| `group_many` | `player_ids` or `members` (required), `target_player` optional | `players/cmd/group_many` |
| `ungroup` | none | `players/cmd/ungroup` |
| `set_members` | `player_ids_to_add` and/or `player_ids_to_remove`, `target_player` optional | `players/cmd/set_members` |
| `group_volume` | `volume_level` or `volume` (0–100) | `players/cmd/group_volume` |
| `next` | none | `play_episode` of the next PinePods item (see Next episode) |
| `sleep_timer` | `mode`: `off`, `minutes` (with `minutes`, 1–720), `end_of_episode`, or `episodes` (with `episodes`, 1–10) | none until it fires (see Sleep timer) |

Transport args match the existing relay: `seek` uses `position_sec` (or `position`), `skip_forward` defaults to 15 seconds, `skip_back` defaults to 15. `set_speed` does not change the speaker.

### Snapshot fields

`NowPlayingSnapshot` gains optional fields. They are omitted when absent, so older clients can ignore them.

| Field | Meaning |
| --- | --- |
| `volume_level` | 0–100 |
| `volume_muted` | bool |
| `ma_player_id` | raw MA player id |
| `group_members` | member player ids (empty array when the player is not a group or sync leader) |
| `sleep_timer` | the running timer: `{"mode":"minutes","ends_at":<unix seconds>}`, `{"mode":"end_of_episode"}`, or `{"mode":"episodes","left":<n>}`; omitted when off |

`episode_id` is `0` until the queue is playing a PinePods stream URL. Title, artwork, and duration are filled from episode metadata once that URL is seen. `playing` is true for MA states `playing` and `buffering`.

Those snapshots are registered on the relay so commands and the MA-Speakers panel can read them. The device-list frame splits them out of the other-devices list: `devices` is other PinePods clients, and `ma_devices` is every Music Assistant player. `GET /api/data/now_playing_devices` uses the same split. Web and mobile read `ma_devices` for the panel and the now-playing bar. A client that only lists `devices` does not show speakers there.

A Sonos speaker often reports `playback_state: idle` while a Music Assistant queue is still loaded: the Sonos provider rewrites a pause as idle, a sync member reports idle while its coordinator plays, and a coordinator change moves `active_source` (which Music Assistant sets to null while its own queue is the source). PinePods keeps the loaded episode across those updates. An idle report does not clear the stream and does not force `playing` off. An explicit `paused` sets `playing` to false and keeps the stream. A Sonos pause is a stop, and its elapsed time falls back to where the stream started (a resume or seek offset). While a queue is not playing, PinePods shows and saves Music Assistant's `resume_pos` instead, which Music Assistant sets to the real position when a playing queue pauses or stops, and holds it until the queue plays again. A null `current_item` or `current_media` is a real stop and clears it. A partial player update with a missing or null state keeps the previous state. An unavailable player stays registered while an episode is loaded. `player_removed` still drops it. The speaker list refreshes every 12 seconds. Play, pause, and stop stay on the row while that player has a loaded episode or was playing, buffering, or paused in the last 60 seconds, including when a refresh reports idle, some other state, or unavailable. An unavailable row says Reconnecting. An unrecognized state says Unknown. A PinePods websocket reconnect does not clear the device list the bar is reading.

## Progress

The session listens for `player_updated`, `player_added`, `player_removed`, `queue_updated`, and `queue_time_updated`, and refreshes `players/all` every 20 seconds (inside the 45 second snapshot TTL).

Music Assistant 2.10.4 does not send `queue_time_updated` every second. It sends it around a seek, or when a player corrects its clock. A Chromecast in flow mode can play for minutes without a queue event. The bridge therefore projects the position itself, the same way the Music Assistant frontend does: the last `elapsed_time`, plus the time since `elapsed_time_last_updated`, times the playback speed. A one-second clock re-anchors each playing queue every 10 seconds, writes history from that position, and republishes the snapshot, so clients (which stop extrapolating after 30 seconds) keep moving.

Listen position is written with `record_podcast_history`, which stores the absolute position (a seek backward sticks). Writes happen on the first sample, every 10 seconds, on pause, on episode change, and when the position jumps by 8 seconds or more. Played count and listen time follow the same samples. A stream URL whose `user_id` does not match the session is ignored. When the remaining time is within the user's auto-complete threshold, the episode is marked completed.

Right after `play_media`, Music Assistant can still report the previous item for a moment. The bridge remembers the URL it sent. A queue event whose `current_item` does not mention that URL is about another item: it drops the loaded episode without writing history, and does not credit that item's position to the new episode. A bare `queue_time_updated` number says nothing about the item, so it keeps the episode already loaded. An episode change also drops the previous episode's duration, so the old length cannot complete the new episode.

## Next episode

PinePods, not Music Assistant, picks what plays next, in the same order as local web:

1. The next playlist episode, when the episode was started with `playlist_id`. The playlist context carries on to that episode.
2. The next episode of the same podcast, when that podcast has auto-play-next. YouTube videos skip this step.
3. The next unfinished episode in the PinePods queue, after the current one.

`next` then starts that episode on the same leader queue with the `play_episode` rules: the saved position, raised to `start_skip`. It does not mark the current episode complete, the same as the local skip. The queue step takes the row after the current one, so a skipped episode that stays queued does not come back.

The web now-playing bar shows a Next button when the device advertises `next`; the 15 second skips use rewind and fast-forward icons. Sending an episode to a speaker adds `playlist_id` only when that episode is the one loaded locally and that playback came from a playlist. When nothing is up next, the bar shows an info message instead of an error.

While the bar follows a speaker, the keyboard controls the speaker with the same keys as local web: Space plays or pauses, and the left and right arrows skip 15 seconds. Keys typed into a text field, and keys held with Alt, Ctrl or Meta, are left alone. The local player's own key handler ignores these keys while the bar follows a speaker, so one key press acts on the speaker only.

## Queue advance

When an episode finishes on a speaker, the bridge plays the next one, so a speaker keeps going after the browser tab closes. An episode finishes when either:

- it is playing and passes `duration - end_skip` (the podcast outro), as on local web; or
- the queue goes from playing to stopped within 5 seconds of the end (or of the outro), or its position jumps from there back to 0 while it still reports playing. Music Assistant 2.10.4 on a Chromecast sends that 0 about a second before the idle state, so the position projected just before the jump decides. A pause earlier in the episode never counts, including a Sonos pause reported as idle.

The next item is read first, in the order above, because `mark_episode_completed` takes the finished episode out of the PinePods queue. Then the finished episode is marked complete, once, and the next one starts the same way as `next`. With nothing next, an outro pauses the speaker and a natural end leaves it stopped. History is not written for the finished episode after that, so the 0 Music Assistant reports at the end does not overwrite completion. Playing it again from earlier in the episode resumes history writes, once it is past the first second: the bare 0 a Chromecast reports at the end while still playing is not a replay.

`end_skip` is read when the episode starts. Playing the same episode again on the same speaker keeps the value read then.

The next item is started by the bridge, not added to Music Assistant's own queue. `play_media` has no start offset, so an enqueued item could not resume or skip its intro without a second seek, and an enqueued remote URL would not map back to its PinePods episode.

## Sleep timer

The bridge runs the sleep timer, so it works with the browser closed. The modes match mobile:

- `minutes`: at `ends_at` the bridge writes the projected position to history and pauses the leader queue. If the speaker is already paused then, the timer just ends.
- `end_of_episode`: when the episode finishes (see Queue advance) it is marked complete and the next one does not start. An outro pauses the speaker; a natural end leaves it stopped.
- `episodes`: each finished episode counts down `left`; the last one behaves like `end_of_episode`. One episode is sent back as `end_of_episode`.

The timer is kept per leader queue in memory. It is cleared by `off`, when it fires, and on a real stop. An API restart drops it. Setting it updates the speaker's snapshot right away.

## Versions

| Server | Result |
| --- | --- |
| 2.10.x | connected, no warning |
| 2.6–2.9 | connected, warning |
| older than 2.6 | refused |
| 3.x or newer | connected, warning |
| missing or unparsable version | connected, warning |
| `schema_version` below 20 | connected, warning added |

A refused version does not register players. The session retries with backoff (2 seconds, doubling up to 60 seconds).

## Trying it against a real 2.10 server

1. Run migrations so `064` creates `MusicAssistant`. `063` is intentionally unused here.
2. In Music Assistant: Settings → Profile, create a long-lived access token.
3. Pick a PinePods origin that the MA host can fetch. `http://127.0.0.1:…` will be rejected.
4. Save settings:

```bash
curl -sS -X PUT "$PINEPODS/api/data/music_assistant/settings" \
  -H "Api-Key: $API_KEY" -H "Content-Type: application/json" \
  -d '{"user_id":1,"enabled":true,"server_url":"http://192.168.1.20:8095","external_base_url":"http://192.168.1.10:8040","token":"'"$MA_TOKEN"'"}'
```

5. Probe first if you want, with `POST /api/data/music_assistant/test` and the same URL and token. Then `GET /api/data/music_assistant/players?user_id=1` until `connected` is true.
6. In the web or mobile app, open Settings and turn on Music Assistant, or save with the curl above. After `connected` is true, the MA-Speakers panel lists players and groups. Play, volume, mute, and grouping are on that panel and on the now-playing bar while an MA device has an episode. Those players do not appear under Other devices. The same actions work over `POST /api/data/music_assistant/command` and over the now-playing relay.

MA must be able to open the stream URL. If playback starts and immediately stops, check that `external_base_url` is reachable from the MA host and that the episode's podcast is the one the scoped RSS key allows.

## Deferred

- No CI run against a live Music Assistant. Unit tests drive a mocked WebSocket. The web and mobile apps send these routes and relay actions; they do not open a Music Assistant socket themselves.
- No `player_queues/get` poll. Progress comes from queue events, the bridge's projection between them, and the 20 second player refresh.
- No power, source select, or MA library sync.
- Playback speed is not offered for Music Assistant speakers. Local playback and other remote devices still use it.
- Seek right after `play_media` can race; a failed seek is a warning.
- Queue advance has a short gap between episodes while the next stream loads. It is not gapless.
- Each API replica opens its own MA session. A command runs on the replica that holds the commanding client. Progress writes are absolute, so overlapping replicas do not stack time. There is no lock electing a single connection.
- YouTube uses the same stream proxy (`type=youtube`) and is not covered by a separate test.
