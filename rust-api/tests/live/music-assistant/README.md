# Live test against a real Music Assistant

The unit tests drive a mocked Music Assistant socket. This runs the bridge against the real thing: a Music Assistant server, two squeezelite players that play into PulseAudio null sinks (so playback runs in real time), a feed server with three six-minute tone episodes, Postgres, Valkey and the PinePods API, all on one Docker network. CI does not run it. It needs only Docker, and about 12 minutes for the whole suite.

```sh
# from this directory
./up.sh infra          # Music Assistant, speakers, feeds, databases
./up.sh api            # in a second terminal: migrate, build and run the API from this checkout
./run.sh setup         # onboard Music Assistant, create the PinePods admin, subscribe, connect the bridge
./run.sh all           # or test names: ./run.sh resume_saved empty_queue
./up.sh down
```

`MA_IMAGE` picks the Music Assistant version (default `ghcr.io/music-assistant/server:2.10.13`). `RUST_LOG` is passed to the API; the default logs the bridge at debug.

Each check compares three things once a second: Music Assistant's own queue position, the bridge's snapshot from `GET /api/data/now_playing_devices`, and the saved listen position.

| Test | What it covers |
|---|---|
| `empty_queue` | First play on a queue that holds nothing, with and without a resume point |
| `cold_start` | First play after the API starts |
| `play_from_start` | Position, snapshot and history while playing from 0 |
| `resume_saved`, `resume_explicit` | Five resumes each, from the saved position and from `position_sec` |
| `resume_back_to_back` | A second `play_episode` right behind the first |
| `transport` | Pause, play, seek, both skips, a seek backward, stop |
| `volume` | Volume, and a mute the player cannot do |
| `next`, `advance`, `end_no_next` | `next`, the queue advancing at the end of an episode, the last episode ending |
| `sleep_end_of_episode`, `sleep_minutes` | Both sleep timer modes |
| `download_stream` | A server download played through the PinePods stream URL |
| `group` | Grouping two players, playing on the leader, ungrouping |
| `errors` | Unknown episode, action and player, a localhost external URL, a wrong token |
| `stall_probe` | Music Assistant alone: `replace` + seek against `add` + `play_index` |

What it found on 2026-11-25, against Music Assistant 2.10.13: the first play on an empty queue lost its resume point and its episode; a `replace` followed by a seek stalled about one start in eight; snapshots stepped back between anchors; and the connection test accepted a wrong token about half the time. The fixes are in `rust-api/src/services/music_assistant.rs`.

`CACHE` names the Docker volumes for the cargo registry, the build and the Python environment (default `pp-live`).

Limits: squeezelite players have no mute in Music Assistant, and a null sink is not a Chromecast or a Sonos. The PinePods API is reached at `http://pp-live-api:8032` without nginx. The feed host is allowed through `PINEPODS_FEED_ALLOWED_HOSTS`.
