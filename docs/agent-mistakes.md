# Agent mistakes

Repeatable mistakes reviewers catch in this repository. Read this file before starting work. Add an entry when a reviewer catches a mistake that should not be repeated.

## Entry format

- **Date:** YYYY-MM-DD
- **PR:** link to the pull request
- **Mistake:** what went wrong
- **Rule going forward:** what to do instead

## Entries

- **Date:** 2026-11-05
- **PR:** none (caught during local verification)
- **Mistake:** Ran `docker build -f dockerfile .` cold. BuildKit builds the `builder` stage (web, `trunk build --release` with `lto = true`) and the `rust-api-builder` stage at the same time. rustc ran out of memory and was killed (`signal: 9, SIGKILL`), and the build failed after about 25 minutes.
- **Rule going forward:** Never build the heavy stages in parallel. Build them one at a time: `docker build --target builder -f dockerfile .`, then `docker build --target rust-api-builder -f dockerfile .`, then the full `docker build -f dockerfile -t pinepods-current:test .`, which reuses both cached stages. Do not run a docker build alongside another cargo or wasm-pack job either.

- **Date:** 2026-11-17
- **PR:** https://github.com/brdweb/PinePods-extra/pull/47 (caught by the maintainer on production)
- **Mistake:** The upstream sync resolved a conflict at the end of `web/static/styles.css` and dropped the closing `}` of `.p2-address`. Every rule after it became nested inside `.p2-address` and stopped applying: the redesigned player (play button, seek bar, speed pill, mini bar layout) and transcript search lost their styles, and no build or test failed.
- **Rule going forward:** After resolving a conflict in a stylesheet, check that every block is closed. `web/src/tests/stylesheet.rs` now fails on a `{` that is never closed.
