#!/usr/bin/env bash
# A real Music Assistant server, two software speakers, a feed server and the databases for a
# live PinePods <-> Music Assistant test, all on one Docker network. See README.md.
#   up.sh infra     start everything except the PinePods API
#   up.sh api       migrate the database and run the PinePods API from this checkout (foreground)
#   up.sh down      remove it all
set -uo pipefail
NET=pp-live
SRC=${SRC:-$(cd "$(dirname "$0")/../../../.." && pwd)}
LIVE=${LIVE:-/tmp/pinepods-live}
MA_IMAGE=${MA_IMAGE:-ghcr.io/music-assistant/server:2.10.13}
# Docker volumes that keep the cargo registry, the build and the Python environment between runs.
CACHE=${CACHE:-pp-live}

down() {
  docker rm -f pp-live-api pp-live-ma pp-live-player1 pp-live-player2 pp-live-feeds pp-live-pg pp-live-valkey >/dev/null 2>&1
  docker network rm $NET >/dev/null 2>&1
  docker volume rm pp-live-ma-data >/dev/null 2>&1
  true
}

infra() {
  down
  docker network create $NET >/dev/null
  mkdir -p $LIVE/feeds $LIVE/downloads
  # Three 6-minute episodes: a tone whose pitch differs per episode. nginx serves them with Range.
  if [ ! -f $LIVE/feeds/ep3.mp3 ]; then
    docker run --rm -v $LIVE/feeds:/out alpine:3.22 sh -c '
      apk add -q --no-cache ffmpeg >/dev/null
      for n in 1 2 3; do
        ffmpeg -loglevel error -y -f lavfi -i "sine=frequency=$((200 + n * 110)):duration=360" -ac 1 -b:a 64k /out/ep$n.mp3
      done'
  fi
  {
    echo '<?xml version="1.0" encoding="UTF-8"?>'
    echo '<rss version="2.0" xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd"><channel>'
    echo '<title>Live Speaker Show</title><link>http://pp-live-feeds/</link><description>Tones for the live test</description><language>en</language>'
    for n in 3 2 1; do
      size=$(stat -c %s $LIVE/feeds/ep$n.mp3)
      echo "<item><title>Episode $n</title><guid isPermaLink=\"false\">live-ep-$n</guid><pubDate>Mon, 0$n Nov 2026 10:00:00 GMT</pubDate>"
      echo "<description>Tone $n</description><itunes:duration>360</itunes:duration>"
      echo "<enclosure url=\"http://pp-live-feeds/ep$n.mp3\" length=\"$size\" type=\"audio/mpeg\"/></item>"
    done
    echo '</channel></rss>'
  } > $LIVE/feeds/feed.xml
  docker run -d --name pp-live-feeds --network $NET -v $LIVE/feeds:/usr/share/nginx/html:ro nginx:alpine >/dev/null
  docker run -d --name pp-live-pg --network $NET --tmpfs /var/lib/postgresql/data \
    -e POSTGRES_USER=pinepods -e POSTGRES_PASSWORD=testpass -e POSTGRES_DB=pinepods_database postgres:17-alpine >/dev/null
  docker run -d --name pp-live-valkey --network $NET valkey/valkey:8-alpine >/dev/null
  docker run -d --name pp-live-ma --network $NET -v pp-live-ma-data:/data $MA_IMAGE >/dev/null
  # Speakers: squeezelite playing into a PulseAudio null sink, which runs in real time.
  for n in 1 2; do
    docker run -d --name pp-live-player$n --network $NET debian:bookworm-slim bash -c '
      apt-get update -qq >/dev/null && apt-get install -qy squeezelite pulseaudio libasound2-plugins >/dev/null 2>&1
      useradd -m speaker
      su speaker -c "pulseaudio --start --exit-idle-time=-1 && sleep 2 && exec squeezelite -o pulse -s pp-live-ma -n \"Live Speaker '$n'\" -m 02:00:00:00:00:0'$n'"' >/dev/null
  done
  docker ps --format '{{.Names}} {{.Status}}' | grep pp-live
}

api() {
  docker rm -f pp-live-api >/dev/null 2>&1
  docker run --rm --name pp-live-api --network $NET --memory 9g -e CARGO_BUILD_JOBS=4 \
    -v $SRC:/src -v $CACHE-cargo:/usr/local/cargo/registry -v $CACHE-target:/target -v $CACHE-venv:/venv \
    -v $LIVE/downloads:/opt/pinepods/downloads \
    -e DB_TYPE=postgresql -e DB_HOST=pp-live-pg -e DB_PORT=5432 -e DB_USER=pinepods -e DB_PASSWORD=testpass \
    -e DB_NAME=pinepods_database -e VALKEY_HOST=pp-live-valkey -e VALKEY_PORT=6379 \
    -e HOSTNAME=http://pp-live-api:8032 -e PINEPODS_PORT=8032 -e DEFAULT_LANGUAGE=en \
    -e SEARCH_API_URL=http://127.0.0.1:9/search -e PEOPLE_API_URL=http://127.0.0.1:9 \
    -e PINEPODS_FEED_ALLOWED_HOSTS=pp-live-feeds -e RUST_LOG="${RUST_LOG:-info,pinepods_api::services::music_assistant=debug}" \
    -e CARGO_TARGET_DIR=/target \
    rust:1.99-bookworm bash -c '
      set -e
      export DEBIAN_FRONTEND=noninteractive
      apt-get update -qq >/dev/null
      apt-get install -qy python3 python3-pip ffmpeg >/dev/null 2>&1
      pip3 install -q --break-system-packages uv==0.13.6 2>/dev/null
      cd /src && UV_PROJECT_ENVIRONMENT=/venv uv sync --locked --quiet
      /venv/bin/python startup/setup_database_new.py 2>&1 | tail -2
      cd /src/rust-api && exec cargo run --quiet'
}

case "${1:-}" in
  infra) infra ;;
  api) api ;;
  down) down ;;
  *) echo "usage: up.sh infra|api|down" >&2; exit 2 ;;
esac
