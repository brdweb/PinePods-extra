# Value for Value

Opt-in Lightning payments from the listener's own wallet. PinePods does not
hold keys and does not send anything until the listener turns the feature on
and sets a daily limit above zero.

The wallet is a [Nostr Wallet Connect](https://github.com/nostr-protocol/nips/blob/master/47.md)
string (`nostr+walletconnect://...`). It is encrypted with the same Fernet key
as other secrets and is never returned by the API. A blank save keeps the
stored string. "Remove wallet string" deletes it.

## What gets paid

Recipients come from the episode `podcast:value` block stored with the feed.
When the episode has no value block, the channel block is used. The client
cannot supply addresses.

- `node`, `keysend`, and `pubkey` are keysend payments. The boostagram TLV
  (`7629169`, UTF-8 JSON, hex on the wire) and the keysend preimage TLV
  (`5482373484`) are both set. `customKey` / `customValue` are an extra TLV:
  an even-length hex value is sent as-is, anything else is hex-encoded UTF-8.
- `lnaddress`, `lightning`, and `lnurl` are lightning addresses. The lookup
  URL and the callback are checked with the public-URL guard before either
  request. Redirects are checked the same way. If the boostagram does not fit
  in `commentAllowed`, a short comment is sent instead and the invoice is
  still paid.
- Any other type is skipped. Its split is not paid to someone else.

Fees (`fee="true"`) are a percent of the original amount and are capped so
they cannot exceed it. A `valueTimeSplit` that contains the playhead then
takes `remotePercentage` of what remains (100 when that split lists recipients
and omits the percentage). Those sats go to the recipients inlined on the
split. A split that only names a `remoteItem` keeps the sats on the local
recipients: the remote feed is not fetched. The other non-fee recipients
share what is left by weight. Weights that do not sum to 100 are normalized.
A zero weight leaves that part unsent.

A boost above the max is rejected. It is not reduced and sent. Streaming uses
sats per minute, independent of the max boost, and still counts against the
daily limit. The limit resets at 00:00 UTC.

Streaming claims the playback minute in the database before paying. A failed
payment deletes that row so a later tick can retry. A crash after a successful
pay can skip a retry of that minute; it does not pay the minute twice.

## API

All routes are under `/api/data` and use the `Api-Key` header.

| Method | Path | Behavior |
| --- | --- | --- |
| GET | `/value/settings?user_id=` | Limits and whether a wallet string is stored. No secret. |
| PUT | `/value/settings` | Save the switch, limits, and an optional new wallet string. |
| POST | `/value/test` | Read the wallet balance. Sends nothing. |
| GET | `/value/episode?user_id=&episode_id=` | Whether the stored block can be paid, plus the suggested amount. |
| POST | `/value/boost` | Send `sats` once. Real money. |
| POST | `/value/stream` | Send the per-minute amount for `position_seconds / 60`. HTTP 200 with `sent: false` when streaming is off, so the player stays quiet. |

Migration `065` creates `ValueWallet` and `ValueStreamMinute`. No row means
the feature is off.

## Trying it

1. Run migration 065 (it runs on server start with the others).
2. In settings, paste a wallet connection string from a wallet you control,
   set a small daily limit and a max boost, and turn **Send sats** on.
3. Use **Test wallet**. It shows a balance and does not pay.
4. Open an episode whose feed has a lightning value block. Confirm the
   checkbox and use **Send real sats**. The show should receive the split.
5. Set sats per minute above 0, play the episode for a minute, and confirm
   one payment for that minute. Seeking back does not pay the same minute again.

Use a wallet and amounts you can afford to lose. Tests mock the wallet and
do not pay.
