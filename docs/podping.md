# Podping

Podping is a stream of Hive notifications that a podcast feed changed. PinePods
can watch that stream and refresh a subscribed feed when its URL appears.
The listener is off unless the server is started with `PINEPODS_PODPING=1`
(`true`, `on`, and `yes` also turn it on). Leaving the variable unset keeps
the normal refresh schedule and does not contact Hive.

`PINEPODS_PODPING_API` is the Hive API the listener calls. It defaults to
`https://api.hive.blog`. The URL is checked with the public-URL guard before
every request, so a private or loopback address is refused. Changing either
variable needs a restart.

A notification refreshes a feed only when that URL is already stored for a
subscription that still has refresh turned on. Other Hive URLs are ignored.
The same feed is not refreshed again for 60 seconds. On startup the listener
starts at the current Hive head, so it does not replay older notifications.
If it falls more than 20 blocks behind, it catches up the newest 20 and leaves
the rest to the regular refresh. A feed shared by several users is fetched once.

`podcast:podping` on a feed is still only a stored tag. The listener matches
the feed URL, whether or not that tag is present.

## Trying it

1. Subscribe to a show and confirm its feed URL in the database matches the URL
   the publisher writes to Podping (host case and a trailing slash do not matter).
2. Set `PINEPODS_PODPING=1` on the server and restart it. The log says the
   listener is on. Without that variable the log says it is off.
3. Publish a Podping update for that feed, or wait for the publisher to send
   one. Within about a minute the feed refreshes without using **Refresh**.
4. A second notification for the same feed inside 60 seconds does not fetch it
   again. A URL that belongs to no subscription does not start a fetch.
