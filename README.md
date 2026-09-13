# Purgent

A unified, cross-platform desktop application combining standards-compliant secure data erasure
(NIST SP 800-88 Rev.1, DoD 5220.22-M, IEEE 2883-2022) and forensic-grade file recovery, with a
single tamper-evident audit trail across both operations.

> **Disclaimer:** Purgent does not itself authorize any action. The operator is solely responsible
> for having the legal right to erase or analyze the target device/media.

## Status

Current phase: **11 — Supabase cloud sync (optional layer) + Auth + RLS** (checkpoint verified:
the app is strictly offline-first — every operation completes and records a signed, verifiable
report locally first, and only then mirrors *audit metadata* (report row: operation info, target,
standard, timestamps, verification status, report hash, HMAC signature) to Supabase when the user
enables sync. Sync is opt-in, gated on `PURGENT_SUPABASE_URL` + `PURGENT_SUPABASE_ANON_KEY` env
vars, and never transmits recovered file contents. Tests mock the REST transport to verify
metadata-only upload, dedup (already-synced reports skipped), retry (failed sync leaves reports
queued), and that an unreachable/absent network leaves the local operation + report fully intact.
Windows: 48 core tests pass, `cargo fmt --check` clean, full workspace builds, frontend builds).

| Phase | Status |
| --- | --- |
| 1 | Complete — checkpoint passed |
| 2 | Complete (Windows) — Linux half verified in Phase 10 |
| 3 | Complete (Windows image-file targets) — live-device wiping exercised in Phase 10 |
| 4 | Complete |
| 5 | Complete |
| 6 | Complete |
| 7 | Complete |
| 8 | Complete |
| 9 | Complete |
| 10 | Complete |
| 11 | Complete |
| 12–15 | Not started |

Phases 2 and 10 provide the cross-platform story: the primary desktop target is Windows (x64);
Linux (x64) is the second target. Ubuntu CI (`.github/workflows/ci.yml`) runs the full core-engine
test suite, the formatting check, and the frontend build on every push/PR so the Linux build is
re-verified continuously.

## Architecture

- `crates/purgent-core` — Rust core engine (no GUI dependency). Storage abstraction, drive/file
  erasers, carving & recovery, hashing, reporting modules.
- `src-tauri` — Tauri v2 native shell. Exposes core-engine functions to the UI via IPC commands.
- `src/` — React frontend rendered in the Tauri webview.

See `PROJECT.md`, `AGENTS.md`, `RULES.md`, and `DECISIONS.md` in the repo root for full
specification and build rules.

## Prerequisites

- Node.js 20+ and npm
- Rust toolchain (stable)
- Windows: MSVC C++ Build Tools (Desktop development with C++) and WebView2 Runtime
- Linux: GTK/WebKit dependencies per Tauri docs

## Development

```sh
npm install
npm run tauri dev
```

## Build & Test

```sh
npm run build                    # frontend build + typecheck
cargo test -p purgent-core       # core engine tests
npm run tauri build              # production bundle
```

## Supported Platforms

Windows (x64) primary; Linux (x64) targeted; macOS per scope in `PROJECT.md` §7.