# AGENTS.md

Instructions for coding agents in this repository (Cursor cloud agents, Codex, Claude Code). The maintainer reviews and merges.

Personal or machine-specific notes never go in tracked files. Claude Code auto-loads the gitignored `CLAUDE.local.md`; Codex users put personal guidance in `~/.codex/AGENTS.md`; `AGENTS.local.md` is a gitignored scratch file that tools don't load automatically.

## Project

This repository is a fork of [madeofpendletonwool/PinePods](https://github.com/madeofpendletonwool/PinePods) (GPL-3.0). Pull requests here target `main`. PinePods is a self-hosted podcast server (Postgres, MySQL/MariaDB or SQLite; Valkey or Redis is optional). Package manifests are version 0.9.2.

- `rust-api/` — Rust 2021 Axum API (`pinepods-api`, `rust-version` 1.99). `Cargo.lock` is committed. Schema SQL is `database_functions/`, applied by `startup/setup_database_new.py`. A new SQLite database starts from the generated `database_functions/sqlite_baseline.sql`; a migration that is not in the baseline runs on SQLite only when it is registered with `sqlite=True` and handles `db_type == "sqlite"`. Code outside `database/` reaches all three backends through `dialect_match!`, whose `pg` arm also runs on SQLite.
- `web/` — Yew frontend, Trunk, feature `server_build`. The Tauri shell is `web/src-tauri/`.
- `mobile/` — Flutter app. CI pins Flutter 3.47.9. The mobile app has no Music Assistant features.

This fork adds Podcasting 2.0 on web and mobile (`docs/podcasting2-api.md`) and Music Assistant output on the web only (`docs/music-assistant.md`). Music Assistant targets do not offer playback speed. The web UI hides speed while it is following a Music Assistant speaker. Features are contributed upstream later as separate squashed pull requests. Those upstream pull requests must not include `AGENTS.md`, `CLAUDE.md`, `docs/agent-mistakes.md`, or the `.gitignore` lines for `AGENTS.local.md` and `CLAUDE.local.md`.

## Commands

**API tests.** `.github/workflows/rust-api-tests.yml`, on pull requests that change `rust-api/**`, `database_functions/**`, `startup/setup_database_new.py`, or that workflow. Stable Rust, Python 3.12, one job per database (Postgres, MariaDB, SQLite). Comments are the working directory. CI does not run `cargo fmt` or `cargo clippy`. Without `PINEPODS_TEST_EXTERNAL_SERVICES`, `scripts/integration-tests.sh` starts Postgres, MariaDB, and Valkey (`rust-api/tests/README.md`); `--db postgres` and `--db postgresql` are the same backend.

```sh
# repo root
sudo apt-get update -qq
sudo apt-get install -qy libmariadb-dev
python -m pip install -q -r rust-api/tests/requirements.txt
python -m pip install -q mariadb
# rust-api/
cargo test --lib --bins
PINEPODS_TEST_EXTERNAL_SERVICES=1 PINEPODS_TEST_PYTHON=python scripts/integration-tests.sh --db postgresql
PINEPODS_TEST_EXTERNAL_SERVICES=1 PINEPODS_TEST_PYTHON=python scripts/integration-tests.sh --db mariadb
PINEPODS_TEST_EXTERNAL_SERVICES=1 PINEPODS_TEST_PYTHON=python scripts/integration-tests.sh --db sqlite
```

**OpenAPI.** `.github/workflows/openapi.yml` when `rust-api/**` changes. In `rust-api/`, then the repo root. The job fails unless `git diff --exit-code rust-api/openapi.json` is clean:

```sh
# rust-api/
cargo run --quiet -- --dump-openapi openapi.json
# repo root
npx -y @redocly/cli@latest lint --config rust-api/redocly.yaml rust-api/openapi.json
```

**Web tests.** `.github/workflows/ci.yaml` job `frontend-tests`, every pull request to `main`. Rust 1.99, `rustup target add wasm32-unknown-unknown`, then `cargo binstall wasm-pack -y`. In `web/`:

```sh
RUSTFLAGS="--cfg=web_sys_unstable_apis" wasm-pack test --node -- --features server_build
```

**Web i18n.** `.github/workflows/check-i18n.yml` when a pull request changes `web/src/**/*.rs` or `web/src/translations/en.json`. CI fetches `github.base_ref` (the pull-request base; `main` for pulls into `main`):

```sh
git fetch origin main
git diff origin/main...HEAD -- 'web/src/**/*.rs' | python3 .github/scripts/check_i18n.py --diff
git diff origin/main...HEAD -- 'web/src/**/*.rs' | python3 .github/scripts/check_i18n_coverage.py --diff
```

**Web and API build.** `.github/workflows/backwards-compatibility.yml`, every pull request to `main`, from the repo root:

```sh
docker build -f dockerfile -t pinepods-current:test .
```

`dockerfile` and `dockerfile-arm` compile `web/`, then `rust-api/`, with:

```sh
RUSTFLAGS="--cfg=web_sys_unstable_apis --cfg getrandom_backend=\"wasm_js\"" trunk build --features server_build --release
cargo build --release
```

Release-only `.github/workflows/build-tauri-clients.yml` is not a pull-request check. On Unix, in `web/`: `RUSTFLAGS="--cfg=web_sys_unstable_apis --cfg getrandom_backend=\"wasm_js\"" trunk build --features server_build` (`web/Trunk.toml` sets `release = true`), then `cargo tauri build` in `web/src-tauri`. Windows runs `powershell -ExecutionPolicy Bypass -File .\build.ps1` in `web/` instead of that `trunk` line. `web/dev-info.md` documents `trunk serve --open`. CI does not run it.

**Mobile tests.** `.github/workflows/mobile-tests.yml`, pull requests that change `mobile/**` or that workflow. Flutter 3.47.9. In `mobile/`:

```sh
flutter pub get
flutter analyze --no-fatal-infos --no-fatal-warnings
flutter test
```

Pull-request CI does not build the app. A push to `main` that changes `mobile/**` runs `.github/workflows/android-test-apk.yml`: `flutter pub get` in `mobile/`, then:

```sh
# mobile/android
printf '\npinepodsForkTest=true\n' >> gradle.properties
# mobile/
flutter build apk --release --target-platform android-arm64
```

Tag and release workflows, not pull requests, run `flutter build apk --release --split-per-abi`, `flutter build appbundle --release`, and `flutter build ios --release --no-codesign`.

**Startup tests.** `.github/workflows/startup-tests.yml`, on pull requests that change `startup/**`, `dockerfile`, `dockerfile-arm`, or that workflow. It runs every `startup/tests/*.sh`; each starts its own container, so it needs only Docker. From the repo root:

```sh
bash startup/tests/ytdlp-root-squash.sh
```

**iOS tests.** `.github/workflows/ios-playback-intent-tests.yml` runs `swift test --package-path mobile/ios/PlaybackSupport` on macOS, only in `madeofpendletonwool/PinePods`. The fork gets no hosted macOS runner, so the job reports skipped here.

## Definition of done

- A change may start from a GitHub Issue, but does not have to. The Issue, or the PR body when there is no Issue, states the goal, non-goals, acceptance criteria, and how to prove them.
- The pull-request commands for every surface the change touches pass locally. Paste that output in the PR body. If a command could not be run, name it and why.
- Every bug fix adds a regression test that fails before the fix.
- A change visible in the web or mobile UI includes a screenshot.
- The PR links the Issue, if there is one, and states what was not done.

## Boundaries

- Never push to `main`. Never force-push a shared branch, delete branches or tags, or create tags or releases.
- No major dependency bumps. No edits to `.github/`, release config, auth, `database_functions/` migrations, or secrets, unless the Issue or the maintainer asks.
- Never edit the workflows that publish to DockerHub, the AUR, or Discord: `.github/workflows/docker-publish.yml`, `.github/workflows/nightly-docker-publish.yml`, `.github/workflows/backend-docker-publish.yml`, `.github/workflows/update-aur-package.yml`, and `.github/workflows/notification.yml`.
- Never weaken or delete a test to make CI pass.
- Keep a pull request to about 400 changed lines. Split work that would exceed that.
- When a requirement is ambiguous, ask in the PR instead of guessing.
- Upstream pull requests must not include this file, `CLAUDE.md`, `docs/agent-mistakes.md`, or the local-notes `.gitignore` lines.

## Commits and PRs

Use Conventional Commits (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `build(deps):`, `chore:`). The PR title uses the same form, because squash merges take the title as the commit subject. Never add `Co-Authored-By` or any other AI co-author or attribution trailer to a commit message or a PR body.

Pass `--repo brdweb/PinePods-extra` to every `gh` command so nothing targets upstream. Upstream syncs: fetch `upstream` (`madeofpendletonwool/PinePods`), branch `sync/upstream-YYYY-MM-DD`, `git merge upstream/main`, resolve, run the pull-request commands for every surface the merge touches, and open `chore: sync with upstream PinePods main` against the fork's `main`. Merge it with a merge commit, not a squash, so upstream history stays reachable. Never hand-resolve `database_functions/sqlite_baseline.sql`: take either side, then regenerate it from a freshly migrated Postgres with `python -m database_functions.sqlite_baseline <postgresql dsn>`. The Postgres integration test `sqlite_baseline_matches_the_migrations` fails until it matches. Take either side of `rust-api/openapi.json` and regenerate it the same way.

## Review guidance

No reviewer runs automatically: Codex (`chatgpt-codex-connector`) reviews are turned off. A reviewer should focus on Axum handlers staying in sync with `rust-api/openapi.json`, Postgres and MariaDB differences, Podcasting 2.0 on both web and mobile, and Music Assistant on the web only (no mobile UI, no playback speed while a Music Assistant speaker is the target, tokens never returned). Claude Code is optional: someone calls it in with `@claude`, where configured, for risky PRs only, and it never runs automatically. The agent that opened the PR fixes review findings. The maintainer merges.

## Review and merge policy

- Bots never post GitHub approvals.
- A PR merges only after CI is green and every review thread is fixed and resolved.
- Every merge here needs the maintainer's explicit approval.
- Bots and agents merge only work the maintainer has explicitly approved.
- Major version bumps, auth, payments, database migrations, CI or release configuration, secrets, and large or ambiguous PRs wait for the maintainer.
- Squash merge, except upstream syncs, which use a merge commit. Commits carry no Co-Authored-By or other AI co-author trailers.

## Learning

Read [`docs/agent-mistakes.md`](docs/agent-mistakes.md) before starting. When a reviewer catches a repeatable mistake, add an entry to that file.
