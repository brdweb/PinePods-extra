# Podcasting 2.0 API

Contract for the tags in the [Podcasting 2.0 namespace](https://github.com/Podcastindex-org/podcast-namespace/blob/main/docs/1.0.md) (both namespace URIs, every phase tag in that document, including the deprecated `podcast:images`). The API stores them at feed refresh and serves them from the database. Web and mobile clients should build against this document.

`itunes:*` tags with the same local names (`episode`, `season`, `block`, `image`) are not Podcasting 2.0 and are not returned here.

## Endpoints

Both require the usual API key and that `user_id` owns the row.

### `GET /api/data/fetch_podcasting_2_data`

Query: `episode_id`, `user_id`.

```json
{
  "chapters": [],
  "transcripts": [],
  "people": [],
  "tags": {}
}
```

| Field | Meaning |
| --- | --- |
| `chapters` | The `chapters` array fetched from `tags.chapters_url`. `[]` when the episode has no chapters URL or the file could not be loaded. This is chapter objects, not the tag. |
| `transcripts` | Same array as `tags.transcripts`. Always present, `[]` when there are none. |
| `people` | Resolved credits: this episode's `podcast:person` tags, plus the show's regular hosts when the episode does not name its own hosts. Same credit object as before (`id`, `podpeople_id`, `name`, `img`, `href`, `role`, `roles`, `group`, `scope`, `episode_count`, `sources`, `verified`, `followed`). Not the raw tag. |
| `tags` | Every Podcasting 2.0 tag that was on this `<item>`. Channel-level people are not copied here. See [Item tags](#item-tags). |

### `GET /api/data/fetch_podcasting_2_pod_data`

Query: `podcast_id`, `user_id`.

| Field | Meaning |
| --- | --- |
| `people` | Show regulars (resolved credits). |
| `guests` | Episode-only credits (resolved credits). |
| `podroll` | Same objects as `tags.podroll`. Always present. |
| `funding` | Same objects as `tags.funding`. Always present. |
| `value` | Same objects as `tags.value`. Always present. |
| `guid` | `podcast:guid` text, or null. Also stored on `Podcasts.PodcastGUID`. |
| `tags` | Every channel-level tag, including `liveItem`. See [Channel tags](#channel-tags). |

`podroll`, `funding`, and `value` stay at the top level so existing clients keep working. New clients can read `tags` alone.

### `GET /api/data/episode_media_options`

Query: `episode_id`, `user_id`, optional `prefer` (`primary` or `alt-N`).

Returns a plan built from the RSS enclosure and every `podcast:alternateEnclosure` stored on the episode. No extra database column: the choice is computed each time.

| Field | Meaning |
| --- | --- |
| `options[]` | One entry for the RSS enclosure (`id: "primary"`) and one per alternate (`id: "alt-0"`, `alt-1`, …). IPFS and torrent sources are listed with `playable: false` and `unsupported_reason`. |
| `selected_id` | The version playback starts with. `prefer` wins when that version can be played. Otherwise an alternate with `default: true`, then the RSS enclosure, then the first playable audio, video, HLS, or DASH source. |
| `playback[]` | Playable `http`/`https` sources, selected version first, then the other playable versions. Clients should try the next entry when a source fails. |
| `download[]` | Progressive files only. Empty when the chosen version is HLS, DASH, IPFS, or a torrent and the client asked for that version explicitly. Auto-selection falls through to another progressive file. |

HLS (`kind: "hls"`) is playable in the web player (native on Safari, vendored hls.js elsewhere) and the mobile player. It is not saved as one file. DASH is offered to the browser and is not downloaded. Integrity objects with `type` `sri`, `sha256`, `sha384`, or `sha512` set `verifiable: true`. The hash is checked when a file is downloaded to the server, downloaded on demand, or saved on the phone. Streaming playback does not hash the whole file.

`POST /api/data/download_podcast` and `GET /api/episodes/{id}/download` take optional `media_option_id`. Alternate hosts that differ from the stored episode URL are refused when they point at a private or loopback address. The original RSS enclosure is left unchanged so a homelab feed on the LAN still downloads. A feed username and password are sent only to that original URL, not to an alternate host. Web and mobile remember `prefer` on the device (`pinepods.mediaOption.{episodeId}`); it is not synced.

## JSON shape

Names are snake_case. Attribute names that differ from the spec:

| Spec attribute | JSON field |
| --- | --- |
| `type` (transcript, image, trailer, alternate enclosure) | `mime_type` |
| `type` (value, valueRecipient, integrity) | `type` |
| `feedGuid`, `feedUrl`, `itemGuid` | `feed_guid`, `feed_url`, `item_guid` |
| `startTime`, `duration` | `start_time`, `duration` |
| `remoteStartTime`, `remotePercentage` | `remote_start_time`, `remote_percentage` |
| `customKey`, `customValue` | `custom_key`, `custom_value` |
| `aspect-ratio` | `aspect_ratio` |
| `contentType` | `content_type` |
| `accountId`, `accountUrl` | `account_id`, `account_url` |
| `usesPodping` | `uses_podping` |
| `pubDate` inside `liveItem` | `pub_date` |

Numbers, booleans-as-text, and free text are stored as strings, exactly as written, except:

- `complete`, `default`, `fee`, and `uses_podping` are JSON booleans, and only the literals `true` and `false` count.
- Missing optional attributes are omitted.
- `transcript` always includes `mime_type`, `language`, and `rel`, using `null` when the attribute was absent. Do not treat a missing key as null on a transcript.
- A `value` block always includes `suggested` (`null` when absent) and `recipients` (possibly `[]`). `time_splits` is omitted when empty.

A `podcast:remoteItem` without `feedGuid` is ignored. One tag of a kind that the spec allows only once (`medium`, `license`, `chat`, `locked`, `updateFrequency`, `podping`, `images`, `season`, `episode`, `chapters`, `publisher`) keeps the first one.

`http` and `https` are required for person `img`/`href`, funding `url`, chapters `url`, transcript `url`, image `href`, trailer `url`, license `url`, `accountUrl`, and `feedUrl`. `source` `uri`, `contentLink` `href`, and social `uri` may be other schemes (`ipfs:` is kept). `javascript:`, `data:`, and `vbscript:` are dropped.

## Channel tags

`tags` on the podcast endpoint. All of these are omitted when empty except where noted above.

| JSON | Spec | Parent |
| --- | --- | --- |
| `guid` | `podcast:guid` | channel |
| `locked` | `podcast:locked` (`value` text, `owner`) | channel |
| `funding[]` | `podcast:funding` (`url`, `description`) | channel, item, liveItem |
| `locations[]` | `podcast:location` (`name` text, `rel`, `geo`, `osm`, `country`) | channel, item, liveItem |
| `trailers[]` | `podcast:trailer` (`title` text, `url`, `pubdate`, `length`, `mime_type`, `season`) | channel |
| `license` | `podcast:license` (`identifier` text, `url`) | channel, item |
| `medium` | `podcast:medium` | channel |
| `image[]` | `podcast:image` (`href`, `alt`, `aspect_ratio`, `width`, `height`, `mime_type`, `purpose`) | channel, item, liveItem |
| `images` | deprecated `podcast:images` (`srcset`, plus `sources[]` of `{url, descriptor}`) | channel, item |
| `social_interacts[]` | `podcast:socialInteract` (`protocol`, `uri`, `account_id`, `account_url`, `priority`). `protocol` `disabled` may omit `uri`. | channel, item, liveItem |
| `blocks[]` | `podcast:block` (`value` text, optional `id`) | channel. An item-level `podcast:block` is stored on the item anyway. |
| `txts[]` | `podcast:txt` (`purpose`, `value` text). `purpose="verify"` is the verify token. | channel, item |
| `update_frequency` | `podcast:updateFrequency` (`text`, `complete`, `dtstart`, `rrule`) | channel |
| `podping` | `podcast:podping` (`uses_podping`) | channel |
| `chat` | `podcast:chat` (`server`, `protocol`, `account_id`, `space`) | channel, item, liveItem |
| `podroll[]` | `podcast:podroll` / `podcast:remoteItem` | channel |
| `publisher` | `podcast:publisher` / one `podcast:remoteItem` | channel |
| `remote_items[]` | `podcast:remoteItem` that is a direct child of `channel` (list feeds). Not podroll and not the publisher. | channel |
| `value[]` | `podcast:value` | channel, item, liveItem |
| `live_items[]` | `podcast:liveItem` | channel |

`value[]` objects: `type`, `method`, `suggested`, `recipients[]`, `time_splits[]`.

`recipients[]`: `name`, `type`, `address`, `split`, and when present `custom_key`, `custom_value`, `fee`.

`time_splits[]`: `start_time`, `duration`, and when present `remote_start_time`, `remote_percentage`, `recipients[]`, `remote_item`. A value block that only has time splits is kept.

Sending sats is separate from this document and is off until a listener turns it on. See `docs/value4value.md`.

`remoteItem` object: `feed_guid`, and when present `feed_url`, `item_guid`, `medium`, `title`.

`live_items[]`: `status`, `start`, `end`, and the RSS children captured inside the live item (`title`, `description`, `link`, `author`, `pub_date`, `enclosure_length`, `enclosure_type`). Nested Podcasting 2.0 tags and the enclosure URL are on `tags`, which has the same shape as an episode's `tags`. A live item is not an episode row.

The show endpoint returns live items in this order: `live`, then `pending` (and any other status) by `start`, then `ended`. An `ended` item is omitted when an episode on that show has the same guid (comparison ignores case and surrounding space). `pending` and `live` items stay even when a guid matches, so a show that is still on the air is not hidden by an older episode. An ended item with no guid stays, and its enclosure can be played until the next refresh replaces the live item with a regular episode. The stored channel JSON is unchanged; only this response is filtered.

Clients play `tags.enclosure_url` when it is `http` or `https`. `application/vnd.apple.mpegurl`, `application/x-mpegURL`, and a URL path ending in `.m3u8` or `.m3u` play as HLS. `video/*` and DASH use a video element; other HLS streams use an audio element. DASH is played by the browser's native player. Upcoming items are listed on the show page. The mobile app has no notification plugin, so it lists those events instead of scheduling a local notification.

Channel `podcast:person` tags are not inside `tags`. They are the resolved `people` array. Raw person fields, when they do appear (item `tags.people`, live item `tags.people`), are `name`, `role`, `group`, `img`, `href`.

## Item tags

`tags` on the episode endpoint. Same objects as above, plus:

| JSON | Spec |
| --- | --- |
| `guid` | RSS `guid` (used to match the episode; not a Podcasting 2.0 tag) |
| `enclosure_url` | RSS `enclosure` `url` |
| `people[]` | `podcast:person` on this item only |
| `chapters_url`, `chapters_type` | `podcast:chapters` `url` and `type` |
| `transcripts[]` | `podcast:transcript` (`url`, `mime_type`, `language`, `rel`) |
| `soundbites[]` | `podcast:soundbite` (`start_time`, `duration`, optional `title`) |
| `season` | `podcast:season` (`number` text, `name`) |
| `episode` | `podcast:episode` (`number` text, `display`) |
| `alternate_enclosures[]` | `podcast:alternateEnclosure` |
| `content_links[]` | `podcast:contentLink` (`href`, `text`) |
| `remote_items[]` | `podcast:remoteItem` on the item |
| `funding[]`, `locations[]`, `license`, `image[]`, `images`, `social_interacts[]`, `blocks[]`, `txts[]`, `chat`, `value[]` | same as the channel |

`alternate_enclosures[]`: `mime_type`, `length`, `bitrate`, `height`, `lang`, `title`, `rel`, `codecs`, `default`, `sources[]` (`uri`, `content_type`), `integrity` (`type`, `value`).

## Refresh scheduling

`podcast:updateFrequency` changes only the scheduled refresh (the cron that walks every podcast). It never polls faster than that cron, and a manual refresh ignores it.

| Tag | Next scheduled fetch |
| --- | --- |
| Absent, or text with no `rrule` and `complete` not `true` | Unchanged. The global cron decides. |
| `complete="true"` | Every 7 days. This wins over `rrule`. |
| `rrule` `FREQ` of `HOURLY`, `DAILY`, `WEEKLY`, `MONTHLY`, or `YEARLY`, times `INTERVAL` (default 1) | That gap, but not less than 60 minutes and not more than 30 days. `YEARLY` therefore waits 30 days. |
| `dtstart` in the future | The first wait ends at `dtstart`, or 30 days out, whichever is sooner. Later waits use the `rrule` gap. |

`Podcasts.P20RefreshIntervalMinutes` is the ongoing gap. `Podcasts.P20NextRefreshAt` is the next time the scheduled refresh will consider the feed. A feed that answers 304 still moves that timestamp forward by the stored gap. A failed refresh does not. YouTube and local folders are unchanged.

## Podping

`podcast:podping` is parsed and returned on `tags.podping`. A Hive listener can refresh a subscribed feed when Podping names its URL. It is off unless `PINEPODS_PODPING=1`. See `docs/podping.md`. The scheduled refresh above still runs either way.

## What is not in this API

Tags that appear only in unadopted `proposal-docs` (`bonusItem`, `hive-account-name`, `opensubscribe`, `value-webmonetization`) are not part of the locked namespace and are not parsed. `podcast:txt purpose="verify"` is the verify token.

## Examples

Episode (`tags` is the item document; `chapters` is the fetched file; `people` is resolved and will not match this sample's ids):

```json
{
  "chapters": [{ "startTime": 0, "title": "Intro" }],
  "transcripts": [
    {
      "url": "https://cdn.example.invalid/t/ep.vtt",
      "mime_type": "text/vtt",
      "language": "en",
      "rel": "captions"
    },
    {
      "url": "https://cdn.example.invalid/t/ep.json",
      "mime_type": "application/json",
      "language": null,
      "rel": null
    }
  ],
  "people": [
    { "id": 1, "name": "Ada & Host", "role": "host", "roles": ["host"], "scope": "podcast", "sources": ["feed"], "followed": false }
  ],
  "tags": {
    "guid": "ep-tagged",
    "enclosure_url": "https://cdn.example.invalid/audio/tagged.mp3?x=1&y=2",
    "people": [{ "name": "Gus Guest", "role": "guest", "group": "cast", "img": "https://example.invalid/gus.jpg", "href": "https://example.invalid/gus" }],
    "chapters_url": "https://cdn.example.invalid/chapters.json",
    "chapters_type": "application/json+chapters",
    "transcripts": [
      { "url": "https://cdn.example.invalid/t/ep.vtt", "mime_type": "text/vtt", "language": "en", "rel": "captions" },
      { "url": "https://cdn.example.invalid/t/ep.json", "mime_type": "application/json", "language": null, "rel": null }
    ],
    "soundbites": [
      { "start_time": "73.0", "duration": "60.0" },
      { "start_time": "1234.5", "duration": "42.25", "title": "Rock & Roll" }
    ],
    "season": { "number": "4", "name": "The Pilot" },
    "episode": { "number": "12", "display": "Ch.3" },
    "alternate_enclosures": [
      {
        "mime_type": "audio/mpeg",
        "codecs": "mp4a.40.2",
        "default": true,
        "sources": [
          { "uri": "https://cdn.example.invalid/audio/tagged.mp3" },
          { "uri": "ipfs://bafybeigdyr", "content_type": "audio/mpeg" }
        ],
        "integrity": { "type": "sri", "value": "sha384-ExVqijgYHm15PqQqdXfW95x" }
      }
    ],
    "value": [
      {
        "type": "lightning",
        "method": "keysend",
        "suggested": "0.00000001000",
        "recipients": [{ "name": "Gus", "type": "node", "address": "02ee", "split": "100" }],
        "time_splits": [
          {
            "start_time": "63",
            "duration": "388",
            "remote_start_time": "10",
            "remote_percentage": "40",
            "recipients": [{ "name": "Guest Split", "type": "node", "address": "02ff", "split": "50" }]
          }
        ]
      }
    ],
    "remote_items": [
      { "feed_guid": "b40ffcf7-2c48-5cfe-8daa-b65d766b2c25", "item_guid": "other-item", "medium": "podcast" }
    ]
  }
}
```

The fixture feed also puts `locations`, `license`, `image`, `images`, `content_links`, `social_interacts`, `blocks`, `txts`, `funding`, and `chat` on that item. Any of those that are absent are omitted.

Podcast (`tags` is the channel document; top-level `podroll`, `funding`, and `value` repeat `tags`):

```json
{
  "people": [],
  "guests": [],
  "podroll": [{ "feed_guid": "29cdca4a-32d8-56ba-b48b-09a011c5daa9", "feed_url": "https://example.invalid/rec.xml", "title": "Buzzcast" }],
  "funding": [{ "url": "https://example.invalid/fund", "description": "Support the show" }],
  "value": [
    {
      "type": "lightning",
      "method": "keysend",
      "suggested": "0.00000005000",
      "recipients": [
        { "name": "Ada", "type": "node", "address": "03ab", "split": "90", "custom_key": "696969", "custom_value": "hello" },
        { "name": "Host", "type": "node", "address": "02cd", "split": "10", "fee": true }
      ],
      "time_splits": [
        {
          "start_time": "60",
          "duration": "237",
          "remote_percentage": "95",
          "remote_item": { "feed_guid": "a94f5cc9-8c58-55fc-91fe-a324087a655b", "item_guid": "remote-1", "medium": "music", "title": "Split Song" }
        }
      ]
    }
  ],
  "guid": "917393e3-1b1e-5cef-ace4-edaa54e1f810",
  "tags": {
    "guid": "917393e3-1b1e-5cef-ace4-edaa54e1f810",
    "medium": "music",
    "locked": { "value": "yes", "owner": "owner@example.invalid" },
    "update_frequency": { "text": "Biweekly", "dtstart": "2026-01-05T00:00:00.000Z", "rrule": "FREQ=WEEKLY;INTERVAL=2" },
    "podping": { "uses_podping": true },
    "publisher": { "feed_guid": "003af0a0-6a45-55cf-b765-68e3d349551a", "medium": "publisher", "title": "Publisher Feed" },
    "chat": { "server": "irc.example.invalid", "protocol": "irc", "account_id": "@ada", "space": "#show" },
    "remote_items": [{ "feed_guid": "ff519475-6e90-5231-91a0-37d092088d88", "item_guid": "song-1", "medium": "music", "title": "Track One" }],
    "live_items": [
      {
        "status": "live",
        "start": "2026-10-21T18:00:00.000Z",
        "end": "2026-10-21T19:00:00.000Z",
        "title": "Live Now",
        "link": "https://example.invalid/live",
        "enclosure_length": "312",
        "enclosure_type": "audio/mpeg",
        "tags": {
          "guid": "live-1",
          "enclosure_url": "https://example.invalid/live.mp3",
          "people": [{ "name": "Live Host", "role": "host" }],
          "content_links": [{ "href": "https://example.invalid/live.html", "text": "Listen live" }]
        }
      }
    ]
  }
}
```

`tags` on a real response also includes `locations`, `trailers`, `license`, `image`, `images`, `social_interacts`, `blocks`, `txts`, `podroll`, `funding`, and `value` when the feed has them. The live item's `tags` uses the full item shape (`chapters_url`, `transcripts`, `alternate_enclosures`, `funding`, `chat`, `value`, and the rest).

OpenAPI schemas for both responses are `EpisodeDocument` and `ChannelDocument` in `rust-api/openapi.json`.
