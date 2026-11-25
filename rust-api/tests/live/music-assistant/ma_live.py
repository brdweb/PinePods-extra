#!/usr/bin/env python3
"""Live test: PinePods Music Assistant bridge against a real Music Assistant server.

Standard library only. Run inside the test network:
    python3 ma_live.py setup       # onboard MA, create the PinePods admin, subscribe, connect
    python3 ma_live.py <test> ...  # run named tests (or "all")
State (tokens, ids) is kept in /state/state.json between runs.
"""
import base64
import json
import os
import sys
import time
import urllib.error
import urllib.request

MA = os.environ.get("MA_URL", "http://pp-live-ma:8095")
PP = os.environ.get("PP_URL", "http://pp-live-api:8032")
FEED = os.environ.get("FEED_URL", "http://pp-live-feeds/feed.xml")
STATE_FILE = os.environ.get("STATE_FILE", "/state/state.json")
# Passwords are hashed client-side by the apps; a fixed argon2 hash of "Admin-pass-123!".
ADMIN_USER, ADMIN_PASS = "liveadmin", "Admin-pass-123!"
DURATION = 360.0


def http(method, url, body=None, headers=None, timeout=30):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(url, data=data, method=method)
    req.add_header("Content-Type", "application/json")
    for k, v in (headers or {}).items():
        req.add_header(k, v)
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            raw = resp.read()
            return resp.status, (json.loads(raw) if raw else None)
    except urllib.error.HTTPError as e:
        raw = e.read()
        try:
            return e.code, json.loads(raw)
        except Exception:
            return e.code, raw.decode(errors="replace")
    except (urllib.error.URLError, OSError) as e:
        return 0, str(e)


def load():
    try:
        return json.load(open(STATE_FILE))
    except FileNotFoundError:
        return {}


def save(state):
    os.makedirs(os.path.dirname(STATE_FILE), exist_ok=True)
    json.dump(state, open(STATE_FILE, "w"), indent=1)


S = load()


def ma(command, **args):
    status, body = http("POST", f"{MA}/api", {"command": command, "args": args, "message_id": "live"},
                        {"Authorization": f"Bearer {S['ma_token']}"})
    if status != 200:
        raise RuntimeError(f"MA {command} -> {status} {body}")
    return body


def pp(method, path, body=None):
    return http(method, f"{PP}{path}", body, {"Api-Key": S["pp_key"]})


def pp_ok(method, path, body=None):
    status, out = pp(method, path, body)
    if status != 200:
        raise RuntimeError(f"PinePods {method} {path} -> {status} {out}")
    return out


def wait_for(what, fn, timeout=60, every=0.5):
    end = time.time() + timeout
    last = None
    while time.time() < end:
        last = fn()
        if last:
            return last
        time.sleep(every)
    raise RuntimeError(f"timed out waiting for {what} (last: {last})")


# ---------------------------------------------------------------- setup

def setup():
    global S
    # Music Assistant: first admin, onboarding, long-lived token, squeezelite provider.
    wait_for("MA web server", lambda: http("GET", f"{MA}/auth/providers")[0] == 200, 180, 2)
    if "ma_token" not in S:
        status, body = http("POST", f"{MA}/setup", {"username": "live", "password": "live-pass-123", "device_name": "live-test"})
        assert status == 200 and body.get("token"), f"MA setup: {status} {body}"
        S["ma_token"] = body["token"]
        try:
            ma("config/onboard_complete")
        except RuntimeError as e:
            print("onboard_complete:", e)
        S["ma_long_token"] = ma("auth/token/create", name="pinepods-live")
        save(S)
    providers = ma("config/providers")
    if not any(p.get("domain") == "squeezelite" for p in providers):
        step = ma("config/providers/setup", provider_domain="squeezelite")
        print("squeezelite setup flow:", json.dumps(step)[:300])
        if step.get("flow_id") and step.get("type") not in ("finish", "abort"):
            step = ma("config/flows/submit", flow_id=step["flow_id"], values={})
            print("squeezelite setup flow submit:", json.dumps(step)[:300])
    players = wait_for("two squeezelite players", lambda: [p for p in ma("players/all") if p.get("provider", "").startswith("squeezelite") and p.get("available")] if len([p for p in ma("players/all") if p.get("provider", "").startswith("squeezelite") and p.get("available")]) >= 2 else None, 120, 2)
    S["players"] = sorted(p["player_id"] for p in players)
    info = http("GET", f"{MA}/info")[1] or {}
    print("MA", info.get("server_version"), "schema", info.get("schema_version"), "players", [(p["player_id"], p.get("name") or p.get("display_name")) for p in players])

    # PinePods: first admin, API key.
    wait_for("PinePods API", lambda: http("GET", f"{PP}/api/pinepods_check")[0] == 200, 1800, 5)
    if "pp_key" not in S:
        hashed = os.environ["ADMIN_HASH"]
        status, body = http("POST", f"{PP}/api/data/create_first", {"username": ADMIN_USER, "password": hashed, "email": "live@example.test", "fullname": "Live Admin"})
        assert status == 200, f"create_first: {status} {body}"
        S["user_id"] = body["user_id"]
        basic = base64.b64encode(f"{ADMIN_USER}:{ADMIN_PASS}".encode()).decode()
        status, body = http("GET", f"{PP}/api/data/get_key", headers={"Authorization": f"Basic {basic}"})
        assert status == 200 and body.get("retrieved_key"), f"get_key: {status} {body}"
        S["pp_key"] = body["retrieved_key"]
        save(S)
    uid = S["user_id"]
    if "podcast_id" not in S:
        body = pp_ok("POST", "/api/data/add_podcast", {"podcast_values": {"pod_title": "Live Speaker Show", "pod_artwork": "", "pod_author": "", "categories": {}, "pod_description": "", "pod_episode_count": 0, "pod_feed_url": FEED, "pod_website": "", "pod_explicit": False, "user_id": uid}, "podcast_index_id": 0})
        assert body.get("success"), body
        S["podcast_id"] = body["podcast_id"]
        save(S)
    eps = wait_for("3 episodes", lambda: (lambda e: e if len(e) == 3 else None)(pp_ok("GET", f"/api/data/podcast_episodes?user_id={uid}&podcast_id={S['podcast_id']}&limit=100").get("episodes", [])), 60)
    S["episodes"] = {e["Episodetitle"]: e["Episodeid"] for e in eps}
    save(S)
    print("episodes", S["episodes"])

    out = pp_ok("POST", "/api/data/music_assistant/test", {"user_id": uid, "server_url": MA, "token": S["ma_long_token"], "external_base_url": PP})
    print("MA test:", out)
    assert out["ok"], out
    out = pp_ok("PUT", "/api/data/music_assistant/settings", {"user_id": uid, "enabled": True, "server_url": MA, "external_base_url": PP, "token": S["ma_long_token"]})
    wait_for("bridge connected", lambda: pp_ok("GET", f"/api/data/music_assistant/players?user_id={uid}").get("connected"), 60)
    out = pp_ok("GET", f"/api/data/music_assistant/players?user_id={uid}")
    print("bridge players:", [(p["player_id"], p["name"], p["available"], p["state"]) for p in out["players"]])
    settings = pp_ok("GET", f"/api/data/music_assistant/settings?user_id={uid}")
    assert "token" not in json.dumps(settings).replace("has_token", ""), "settings response leaks the token"
    print("settings:", settings)


# ---------------------------------------------------------------- helpers for tests

def uid():
    return S["user_id"]


def ep(n):
    return S["episodes"][f"Episode {n}"]


def player(n=0):
    return S["players"][n]


def command(action, player_id=None, **args):
    return pp("POST", "/api/data/music_assistant/command", {"user_id": uid(), "player_id": player_id or player(), "action": action, "args": args})


def command_ok(action, player_id=None, **args):
    status, out = command(action, player_id, **args)
    if status != 200:
        raise RuntimeError(f"command {action} -> {status} {out}")
    return out


def queue(player_id=None):
    return ma("player_queues/get", queue_id=player_id or player())


def ma_elapsed(player_id=None):
    """Music Assistant's own idea of the position, projected like its frontend does."""
    q = queue(player_id)
    elapsed = q.get("elapsed_time") or 0
    if q.get("state") == "playing" and q.get("elapsed_time_last_updated"):
        elapsed += time.time() - q["elapsed_time_last_updated"]
    return elapsed, q.get("state"), (q.get("current_item") or {}).get("name")


def snapshot(player_id=None):
    out = pp_ok("GET", f"/api/data/now_playing_devices?user_id={uid()}")
    want = f"ma:{uid()}:{player_id or player()}"
    for d in out.get("ma_devices", []):
        if d["device_id"] == want:
            return d
    return None


def saved_position(episode_id):
    out = pp_ok("POST", "/api/data/get_episode_metadata", {"episode_id": episode_id, "user_id": uid(), "person_episode": False, "is_youtube": False})
    e = out.get("episode", out)
    return e.get("listenduration"), e.get("completed")


def set_position(episode_id, pos):
    # /progress is newest-wins, so it can move a position back; record_podcast_history only moves forward.
    pp_ok("POST", "/api/data/progress", {"client": "web", "updates": [{"episode_id": episode_id, "position": pos}]})


def stop_all():
    for p in S["players"]:
        try:
            command("stop", p)
        except Exception:
            pass
    time.sleep(1.5)


def reset(*episodes):
    """Stop the speakers and clear position and completion for the episodes."""
    stop_all()
    for e in episodes or (ep(1), ep(2), ep(3)):
        pp("POST", "/api/data/mark_episode_uncompleted", {"episode_id": e, "user_id": uid(), "is_youtube": False})
        set_position(e, 0)


RESULTS = []


def check(name, ok, detail=""):
    RESULTS.append((name, bool(ok), detail))
    print(("  PASS " if ok else "  FAIL ") + name + (f" — {detail}" if detail else ""), flush=True)
    return ok


def sample(label, seconds, every=1.0, episode_id=None, player_id=None):
    """Print MA position, bridge snapshot and saved position once a second."""
    rows = []
    t0 = time.time()
    while time.time() - t0 < seconds:
        el, state, item = ma_elapsed(player_id)
        snap = snapshot(player_id) or {}
        saved = saved_position(episode_id)[0] if episode_id else None
        # Clients project the snapshot the same way: position plus the time since it was taken.
        snap_pos = snap.get("position_sec") or 0
        if snap.get("playing") and snap.get("updated_at"):
            snap_pos += max(0.0, time.time() - snap["updated_at"])
        row = (round(time.time() - t0, 1), round(el, 1), state, snap.get("episode_id"), round(snap_pos, 1), snap.get("playing"), saved)
        rows.append(row)
        print(f"    {label} t={row[0]:5.1f} ma={row[1]:6.1f} {row[2]:8} snap_ep={row[3]} snap={row[4]:6.1f} playing={row[5]} saved={row[6]}", flush=True)
        time.sleep(every)
    return rows


# ---------------------------------------------------------------- tests

def t_play_from_start():
    """A fresh episode starts at 0 and the position moves in real time."""
    reset()
    out = command_ok("play_episode", episode_id=ep(1), is_youtube=False)
    print("   ", out)
    rows = sample("start", int(os.environ.get("START_SECS", "20")), episode_id=ep(1))
    last = rows[-1]
    check("fresh episode plays", last[2] == "playing" and 8 <= last[1] <= last[0] + 4, f"MA at {last[1]}s after {last[0]}s")
    check("bridge snapshot follows the speaker", last[3] == ep(1) and abs(last[4] - last[1]) <= 4, f"snapshot {last[4]}s vs MA {last[1]}s")
    check("position is saved while playing", (last[6] or 0) >= 5 and abs((last[6] or 0) - last[1]) <= 15, f"saved {last[6]}s vs MA {last[1]}s")


def t_cold_start():
    """Right after the API starts: the first play on a queue the bridge has not seen play."""
    wait_for("PinePods API", lambda: http("GET", f"{PP}/api/pinepods_check")[0] == 200, 1800, 3)
    wait_for("bridge connected", lambda: pp_ok("GET", f"/api/data/music_assistant/players?user_id={uid()}").get("connected"), 120)
    set_position(ep(1), 0)
    out = command_ok("play_episode", player(1), episode_id=ep(1), is_youtube=False)
    print("   ", out)
    rows = sample("cold", 30, episode_id=ep(1), player_id=player(1))
    last = rows[-1]
    first_ep = next((r[0] for r in rows if r[3] == ep(1)), None)
    check("cold start: the speaker plays", last[2] == "playing" and last[1] >= 15, f"MA at {last[1]}s after {last[0]}s")
    check("cold start: the snapshot names the episode within 5s", first_ep is not None and first_ep <= 5, f"first seen at t={first_ep}")
    check("cold start: snapshot follows the speaker", abs(last[4] - last[1]) <= 4, f"snapshot {last[4]}s vs MA {last[1]}s")
    check("cold start: position is saved", (last[6] or 0) >= 10, f"saved {last[6]}s")


def t_empty_queue():
    """First play on a Music Assistant queue that holds nothing (new speaker, or a cleared queue)."""
    reset()
    for explicit in (None, 100):
        stop_all()
        ma("player_queues/clear", queue_id=player(1))
        time.sleep(2)
        print("    queue before:", {k: queue(player(1)).get(k) for k in ("state", "current_item", "items", "elapsed_time")})
        set_position(ep(2), 0)
        args = {"episode_id": ep(2), "is_youtube": False}
        if explicit is not None:
            args["position_sec"] = explicit
        out = command_ok("play_episode", player(1), **args)
        print("   ", out)
        label = f"empty@{explicit or 0}"
        rows = sample(label, 16, episode_id=ep(2), player_id=player(1))
        last = rows[-1]
        first_ep = next((r[0] for r in rows if r[3] == ep(2)), None)
        check(f"{label}: the speaker plays", last[2] == "playing" and last[1] >= (explicit or 0) + 8, f"MA at {last[1]}s after {last[0]}s")
        check(f"{label}: the snapshot names the episode within 5s", first_ep is not None and first_ep <= 5, f"first seen at t={first_ep}")
        check(f"{label}: position is saved", (last[6] or 0) >= (explicit or 0) + 5, f"saved {last[6]}s")


def advancing(player_id, target, wait=8):
    """After a start at , is the speaker really moving? MA says playing either way."""
    time.sleep(wait)
    q = queue(player_id)
    raw = q.get("elapsed_time") or 0
    return raw - target, q.get("state")


def t_stall_probe():
    """Music Assistant alone: start at an offset by replace+seek, and by add+play_index."""
    stop_all()
    url = "http://pp-live-feeds/ep2.mp3"
    q = player(1)
    rounds = int(os.environ.get("PROBE_ROUNDS", "8"))
    for label in ("replace+seek", "clear+add+play_index"):
        stalled = 0
        for i in range(rounds):
            target = 40 + 25 * i
            ma("player_queues/stop", queue_id=q)
            time.sleep(1)
            if label == "replace+seek":
                ma("player_queues/play_media", queue_id=q, media=url, option="replace")
                for _ in range(40):
                    try:
                        ma("player_queues/seek", queue_id=q, position=target)
                        break
                    except RuntimeError:
                        time.sleep(0.1)
            else:
                ma("player_queues/clear", queue_id=q)
                ma("player_queues/play_media", queue_id=q, media=url, option="add")
                ma("player_queues/play_index", queue_id=q, index=0, seek_position=target)
            moved, state = advancing(q, target)
            ok = state == "playing" and moved >= 4
            stalled += 0 if ok else 1
            print(f"    {label} #{i + 1} @{target}: raw elapsed moved {moved:.1f}s in 8s, state {state}{'' if ok else '  <-- STALLED'}", flush=True)
        check(f"{label}: every start at an offset keeps playing", stalled == 0, f"{stalled} of {rounds} stalled")
    ma("player_queues/stop", queue_id=q)


def resume_run(label, episode_id, target, explicit, settle=25):
    stop_all()
    if not explicit:
        set_position(episode_id, target)
    args = {"episode_id": episode_id, "is_youtube": False}
    if explicit:
        args["position_sec"] = target
    t0 = time.time()
    out = command_ok("play_episode", **args)
    took = time.time() - t0
    rows = sample(label, settle, episode_id=episode_id)
    # The saved position must never fall back to the start of the file.
    lowest_saved = min((r[6] for r in rows if r[6] is not None), default=None)
    last = rows[-1]
    expect_low, expect_high = target + 3, target + settle + 12
    reached = next((r[0] for r in rows if r[2] == "playing" and r[1] >= target), None)
    ok_pos = last[2] == "playing" and expect_low <= last[1] <= expect_high
    check(f"{label}: speaker ends up at the resume point", ok_pos, f"target {target}s, MA at {last[1]}s after {last[0]}s; reached it at t={reached}; command took {took:.1f}s warnings={out.get('warnings')}")
    check(f"{label}: saved position never drops below the resume point", lowest_saved is not None and lowest_saved >= target - 2, f"lowest saved {lowest_saved}s")
    check(f"{label}: snapshot agrees with the speaker", abs(last[4] - last[1]) <= 4, f"snapshot {last[4]}s vs MA {last[1]}s")
    return rows


def t_resume_saved():
    """Saved listen position: play_episode without a position resumes there. Five runs."""
    reset()
    for i, target in enumerate([120, 45, 200, 300, 90]):
        resume_run(f"saved#{i + 1}@{target}", ep(1 + i % 2), target, explicit=False, settle=18)


def t_resume_explicit():
    """position_sec in the command wins over the saved position. Five runs."""
    reset()
    for i, target in enumerate([150, 30, 250, 75, 330]):
        set_position(ep(2), 10)
        resume_run(f"explicit#{i + 1}@{target}", ep(2), target, explicit=True, settle=18)


def t_resume_back_to_back():
    """Starting another episode while a resume seek may still be waiting."""
    reset()
    set_position(ep(1), 200)
    set_position(ep(2), 60)
    command_ok("play_episode", episode_id=ep(1), is_youtube=False)
    time.sleep(0.3)
    command_ok("play_episode", episode_id=ep(2), is_youtube=False)
    rows = sample("b2b", 20, episode_id=ep(2))
    last = rows[-1]
    check("second episode wins and resumes at its own position", last[3] == ep(2) and 60 <= last[1] <= 95, f"snapshot ep {last[3]}, MA at {last[1]}s")
    pos1, _ = saved_position(ep(1))
    check("first episode keeps its saved position", abs((pos1 or 0) - 200) <= 3, f"episode 1 saved {pos1}s")


def t_transport():
    """Pause, play, seek, skip, stop."""
    reset()
    command_ok("play_episode", episode_id=ep(3), is_youtube=False, position_sec=100)
    sample("warm", 10, episode_id=ep(3))
    command_ok("pause")
    time.sleep(3)
    el1, state1, _ = ma_elapsed()
    time.sleep(4)
    el2, state2, _ = ma_elapsed()
    snap = snapshot()
    pos, _ = saved_position(ep(3))
    check("pause holds the position", state2 != "playing" and abs(el2 - el1) < 1.5, f"{el1:.1f}s -> {el2:.1f}s state {state2}")
    check("pause is saved", pos is not None and abs(pos - el2) <= 6 and pos >= 100, f"saved {pos}s vs MA {el2:.1f}s")
    check("snapshot shows paused with the episode kept", snap and snap["episode_id"] == ep(3) and not snap["playing"], f"{snap and (snap['episode_id'], snap['playing'], snap['position_sec'])}")
    command_ok("play")
    rows = sample("resume", 8, episode_id=ep(3))
    check("play resumes from the paused position", rows[-1][2] == "playing" and el2 - 2 <= rows[-1][1] <= el2 + 16, f"paused at {el2:.1f}s, now {rows[-1][1]}s")
    command_ok("seek", position_sec=240)
    rows = sample("seek", 8, episode_id=ep(3))
    check("seek moves the speaker", 238 <= rows[-1][1] <= 256, f"MA at {rows[-1][1]}s")
    check("seek is saved", rows[-1][6] is not None and rows[-1][6] >= 238, f"saved {rows[-1][6]}s")
    before = ma_elapsed()[0]
    command_ok("skip_back", seconds=30)
    time.sleep(4)
    after = ma_elapsed()[0]
    check("skip_back goes back 30s", -30 <= after - before <= -20, f"{before:.1f}s -> {after:.1f}s")
    before = ma_elapsed()[0]
    command_ok("skip_forward", seconds=15)
    time.sleep(4)
    after = ma_elapsed()[0]
    check("skip_forward goes forward 15s", 15 <= after - before <= 25, f"{before:.1f}s -> {after:.1f}s")
    command_ok("seek", position_sec=20)
    rows = sample("seek-back", 6, episode_id=ep(3))
    check("a seek backward sticks in history", rows[-1][6] is not None and rows[-1][6] <= 40, f"saved {rows[-1][6]}s, MA {rows[-1][1]}s")
    command_ok("stop")
    time.sleep(3)
    _, state, _ = ma_elapsed()
    check("stop stops the speaker", state != "playing", f"state {state}")


def t_volume():
    """Volume and mute reach the speaker and come back in the snapshot."""
    reset()
    command_ok("volume_set", volume_level=35)
    time.sleep(1.5)
    p = [x for x in ma("players/all") if x["player_id"] == player()][0]
    check("volume_set reaches the speaker", p.get("volume_level") == 35, f"MA volume {p.get('volume_level')}")
    status, out = command("volume_mute", muted=True)
    if status == 400 and "not supported" in json.dumps(out):
        # Squeezelite players have no mute in Music Assistant. The refusal comes back as it is.
        check("a mute the speaker cannot do is refused with Music Assistant's reason", True, json.dumps(out))
    else:
        time.sleep(1.5)
        p = [x for x in ma("players/all") if x["player_id"] == player()][0]
        check("mute reaches the speaker", status == 200 and p.get("volume_muted") is True, f"{status} MA muted {p.get('volume_muted')}")
        command_ok("volume_mute", muted=False)
    snap = wait_for("snapshot volume", lambda: (lambda s: s if s and s.get("volume_level") == 35 else None)(snapshot()), 30)
    check("snapshot carries the volume", snap["volume_level"] == 35, f"{snap.get('volume_level')} muted {snap.get('volume_muted')}")


def queue_episodes(*ids):
    playing = None
    for e in ids:
        body = {"episode_id": e, "user_id": uid(), "is_youtube": False, "position": "bottom"}
        if playing:
            body["playing_episode_id"] = playing
            body["playing_is_youtube"] = False
        pp("POST", "/api/data/queue_pod", body)
        playing = e


def clear_queue():
    out = pp_ok("GET", f"/api/data/get_queued_episodes?user_id={uid()}")
    for e in out.get("data", out.get("episodes", [])) if isinstance(out, dict) else []:
        pp("POST", "/api/data/remove_queued_pod", {"episode_id": e.get("episodeid") or e.get("Episodeid"), "user_id": uid(), "is_youtube": False})


def t_next():
    """`next` plays the next queued PinePods episode at its saved position."""
    reset()
    clear_queue()
    queue_episodes(ep(1), ep(2), ep(3))
    set_position(ep(2), 150)
    command_ok("play_episode", episode_id=ep(1), is_youtube=False)
    sample("first", 8, episode_id=ep(1))
    out = command_ok("next")
    print("   ", out)
    rows = sample("next", 18, episode_id=ep(2))
    last = rows[-1]
    check("next starts the next queued episode", last[3] == ep(2), f"snapshot episode {last[3]}")
    check("next resumes that episode's saved position", 150 <= last[1] <= 185, f"MA at {last[1]}s")
    _, done = saved_position(ep(1))
    check("next does not mark the skipped episode complete", not done, f"completed={done}")


def t_advance():
    """An episode that plays to its end is completed and the next queued one starts."""
    reset()
    clear_queue()
    queue_episodes(ep(1), ep(2))
    command_ok("play_episode", episode_id=ep(1), is_youtube=False, position_sec=335)
    rows = sample("end", 55, episode_id=ep(2))
    last = rows[-1]
    _, done = saved_position(ep(1))
    check("finished episode is marked complete", bool(done), f"completed={done}")
    check("the next queued episode starts by itself", last[3] == ep(2) and last[2] == "playing", f"snapshot episode {last[3]}, state {last[2]}, MA at {last[1]}s")
    check("the next episode starts from its beginning", last[1] <= 45, f"MA at {last[1]}s")
    pos1, _ = saved_position(ep(1))
    print(f"    episode 1 saved position after completion: {pos1}")


def t_end_no_next():
    """The last episode ends: it is completed and the speaker stops."""
    reset()
    clear_queue()
    command_ok("play_episode", episode_id=ep(3), is_youtube=False, position_sec=340)
    rows = sample("last", 40, episode_id=ep(3))
    _, done = saved_position(ep(3))
    check("last episode is marked complete", bool(done), f"completed={done}")
    check("speaker is not playing afterwards", rows[-1][2] != "playing", f"state {rows[-1][2]}")


def t_sleep_end_of_episode():
    """Sleep timer: end of episode completes it and does not start the next."""
    reset()
    clear_queue()
    queue_episodes(ep(1), ep(2))
    command_ok("play_episode", episode_id=ep(1), is_youtube=False, position_sec=335)
    time.sleep(4)
    command_ok("sleep_timer", mode="end_of_episode")
    snap = snapshot()
    check("snapshot shows the timer", snap and (snap.get("sleep_timer") or {}).get("mode") == "end_of_episode", f"{snap and snap.get('sleep_timer')}")
    rows = sample("sleep", 45, episode_id=ep(2))
    _, done = saved_position(ep(1))
    check("episode is completed", bool(done), f"completed={done}")
    check("next episode does not start", rows[-1][3] != ep(2) and rows[-1][2] != "playing", f"snapshot episode {rows[-1][3]} state {rows[-1][2]}")


def t_sleep_minutes():
    """Sleep timer: one minute pauses the speaker and saves the position."""
    reset()
    command_ok("play_episode", episode_id=ep(2), is_youtube=False, position_sec=50)
    time.sleep(5)
    command_ok("sleep_timer", mode="minutes", minutes=1)
    rows = sample("sleep-min", 70, every=2, episode_id=ep(2))
    last = rows[-1]
    check("speaker pauses when the minute is up", last[2] != "playing", f"state {last[2]} at {last[1]}s")
    check("position saved near the pause", last[6] is not None and 100 <= last[6] <= 125, f"saved {last[6]}s")


def t_download_stream():
    """A server-downloaded episode plays through the PinePods stream URL and resumes."""
    reset()
    pp("POST", "/api/data/download_podcast", {"episode_id": ep(3), "user_id": uid(), "is_youtube": False})
    def downloaded():
        out = pp_ok("GET", f"/api/data/download_episode_list?user_id={uid()}")
        return str(ep(3)) in json.dumps(out) and any(str(v) == str(ep(3)) for row in (out.get("downloaded_episodes") or out.get("episodes") or []) for k, v in row.items() if k.lower() == "episodeid")
    try:
        wait_for("server download", downloaded, 90, 2)
    except RuntimeError as e:
        check("server download finished", False, str(e))
        return
    rows = resume_run("downloaded@210", ep(3), 210, explicit=False, settle=18)
    q = queue()
    uri = json.dumps(q.get("current_item") or {})
    check("downloaded episode is fetched from PinePods", "/api/data/stream/" in uri and "pp-live-api" in uri, uri[:0] or ("stream URL" if "/api/data/stream/" in uri else "direct URL"))
    full_key = S["pp_key"]
    check("the stream URL does not carry the user's API key", full_key not in uri)


def t_group():
    """Group the two speakers, play on the leader, ungroup."""
    reset()
    a, b = player(0), player(1)
    status, out = command("group_many", a, player_ids=[b], target_player=a)
    if status != 200:
        status, out = command("group", b, target_player=a)
    print("    group:", status, out)
    time.sleep(3)
    players = {p["player_id"]: p for p in pp_ok("GET", f"/api/data/music_assistant/players?user_id={uid()}")["players"]}
    check("bridge lists the group", b in (players[a].get("group_members") or []) or players[b].get("synced_to") == a, f"leader members {players[a].get('group_members')} member synced_to {players[b].get('synced_to')}")
    command_ok("play_episode", a, episode_id=ep(1), is_youtube=False, position_sec=80)
    rows = sample("group", 15, episode_id=ep(1), player_id=a)
    check("grouped playback resumes at the position", 80 <= rows[-1][1] <= 110 and rows[-1][2] == "playing", f"MA at {rows[-1][1]}s {rows[-1][2]}")
    command("stop", a)
    status, out = command("ungroup", b)
    time.sleep(3)
    players = {p["player_id"]: p for p in pp_ok("GET", f"/api/data/music_assistant/players?user_id={uid()}")["players"]}
    check("ungroup splits them again", not players[b].get("synced_to") and b not in (players[a].get("group_members") or []), f"{status} {out}; members {players[a].get('group_members')}")


def t_errors():
    """Bad input is refused cleanly."""
    status, out = command("play_episode", episode_id=999999, is_youtube=False)
    check("unknown episode is a 404", status == 404, f"{status} {out}")
    status, out = command("warp_speed")
    check("unknown action is a 400", status == 400, f"{status} {out}")
    status, out = command("play_episode", "no-such-player", episode_id=ep(1), is_youtube=False)
    check("unknown player is refused", status in (400, 404), f"{status} {out}")
    status, out = pp("PUT", "/api/data/music_assistant/settings", {"user_id": uid(), "enabled": True, "server_url": MA, "external_base_url": "http://localhost:8040"})
    check("localhost external URL is refused", status == 400, f"{status} {out}")
    status, out = pp_ok("POST", "/api/data/music_assistant/test", {"user_id": uid(), "server_url": MA, "token": "wrong-token", "external_base_url": PP}), None
    check("a wrong token fails the probe", status.get("ok") is False, f"{status}")
    status, out = command("set_speed", speed=1.5)
    check("set_speed is accepted and ignored", status == 200, f"{status} {out}")


TESTS = {
    "stall_probe": t_stall_probe,
    "empty_queue": t_empty_queue,
    "cold_start": t_cold_start,
    "play_from_start": t_play_from_start,
    "resume_saved": t_resume_saved,
    "resume_explicit": t_resume_explicit,
    "resume_back_to_back": t_resume_back_to_back,
    "transport": t_transport,
    "volume": t_volume,
    "next": t_next,
    "advance": t_advance,
    "end_no_next": t_end_no_next,
    "sleep_end_of_episode": t_sleep_end_of_episode,
    "sleep_minutes": t_sleep_minutes,
    "download_stream": t_download_stream,
    "group": t_group,
    "errors": t_errors,
}


def main():
    names = sys.argv[1:] or ["all"]
    if names == ["setup"]:
        setup()
        return 0
    if names == ["all"]:
        names = list(TESTS)
    for name in names:
        print(f"=== {name}: {(TESTS[name].__doc__ or name).strip()}", flush=True)
        try:
            TESTS[name]()
        except Exception as e:  # keep going: one broken scenario should not hide the rest
            check(f"{name} ran to the end", False, repr(e))
    try:
        stop_all()
    except Exception:
        pass
    failed = [r for r in RESULTS if not r[1]]
    print(f"\n{len(RESULTS) - len(failed)} passed, {len(failed)} failed")
    for name, _, detail in failed:
        print(f"  FAILED {name} — {detail}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
