#!/usr/bin/env bash
# Run ma_live.py inside the test network: run.sh setup, then run.sh all (or test names).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
LIVE=${LIVE:-/tmp/pinepods-live}
mkdir -p "$LIVE/state"
if [ ! -s "$LIVE/state/hash" ]; then
  # The apps hash the password before they send it; the test's admin does the same.
  docker run --rm python:3.12-slim sh -c 'pip install -q argon2-cffi 2>/dev/null && python -c "from argon2 import PasswordHasher; print(PasswordHasher().hash(\"Admin-pass-123!\"))"' > "$LIVE/state/hash"
fi
docker run --rm --network pp-live -v "$HERE:/live:ro" -v "$LIVE/state:/state" \
  -e ADMIN_HASH="$(cat "$LIVE/state/hash")" -e START_SECS="${START_SECS:-20}" -e PROBE_ROUNDS="${PROBE_ROUNDS:-8}" \
  python:3.12-slim python3 -u /live/ma_live.py "$@"
