#!/usr/bin/env bash
# Regression test for seeding yt-dlp on downloads that root may not write (root-squashed NFS,
# upstream madeofpendletonwool/PinePods#1080). Needs Docker. From the repo root:
#   bash startup/tests/ytdlp-root-squash.sh
# Root inside the container runs without DAC_OVERRIDE, FOWNER and CHOWN, so like a root-squashed
# mount it cannot write the 0700 downloads directory owned by the runtime user (911).
set -euo pipefail
repo="$(cd "$(dirname "$0")/../.." && pwd)"

if [[ "${1:-}" != "--inside" ]]; then
    exec docker run --rm -v "$repo:/src:ro" alpine:3.22 sh -c \
        'apk add -q --no-cache bash su-exec setpriv >/dev/null && bash /src/startup/tests/ytdlp-root-squash.sh --inside'
fi

fail() { echo "FAIL: $*" >&2; exit 1; }
as_squashed_root() {
    setpriv --inh-caps=-dac_override,-fowner,-chown --bounding-set=-dac_override,-fowner,-chown "$@"
}
reset_downloads() {
    rm -rf /opt/pinepods/downloads
    mkdir -p /opt/pinepods/downloads
    chown 911:911 /opt/pinepods/downloads
    chmod 700 /opt/pinepods/downloads
}

adduser -D -u 911 pinepods >/dev/null 2>&1
printf '#!/bin/sh\necho "horust uid=$(id -u)"\n' > /usr/local/bin/horust
printf '#!/bin/sh\necho image\n' > /usr/local/bin/yt-dlp
chmod +x /usr/local/bin/horust /usr/local/bin/yt-dlp
export YTDLP_PATH=/opt/pinepods/downloads/.bin/yt-dlp

# startup.sh runs as root: it must leave seeding to the runtime user.
if grep -q 'cp /usr/local/bin/yt-dlp' /src/startup/startup.sh; then
    fail "startup.sh seeds yt-dlp as root; on root-squashed downloads that fails"
fi
[[ -f /src/startup/start-services.sh ]] || fail "startup/start-services.sh is missing"

# The simulation holds: squashed root cannot create the directory.
reset_downloads
if as_squashed_root mkdir -p "$(dirname "$YTDLP_PATH")" 2>/dev/null; then
    fail "the simulation is broken: squashed root could write the downloads directory"
fi

# The runtime user seeds the binary, then Horust starts as that user.
out="$(su-exec 911:911 bash /src/startup/start-services.sh)"
grep -q 'horust uid=911' <<<"$out" || fail "Horust did not start as 911: $out"
[[ "$("$YTDLP_PATH")" == image ]] || fail "yt-dlp was not seeded from the image"
[[ "$(stat -c %u "$YTDLP_PATH")" == 911 ]] || fail "seeded yt-dlp is not owned by the runtime user"

# A restart keeps the self-updated binary.
su-exec 911:911 sh -c "printf '#!/bin/sh\necho updated\n' > '$YTDLP_PATH'"
out="$(su-exec 911:911 bash /src/startup/start-services.sh)"
grep -q Seeding <<<"$out" && fail "restart re-seeded over the self-updated binary"
[[ "$("$YTDLP_PATH")" == updated ]] || fail "restart replaced the self-updated binary"

# Legacy mode (no PUID/PGID) on a local volume still seeds as root.
rm -rf /opt/pinepods/downloads && mkdir -p /opt/pinepods/downloads
out="$(bash /src/startup/start-services.sh)"
grep -q 'horust uid=0' <<<"$out" || fail "legacy mode did not start Horust: $out"
[[ -x "$YTDLP_PATH" ]] || fail "legacy mode did not seed yt-dlp"

echo "ok: yt-dlp seeding works on root-squashed and local downloads"
