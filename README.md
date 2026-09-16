<div align="center">

<img src="src/assets/logo.png" alt="Purgent logo" width="110" />

# Purgent

**Unified cross-platform forensic sanitization & recovery**

Standards-compliant secure data erasure (NIST SP 800-88, DoD 5220.22-M, IEEE 2883-2022) combined with
forensic-grade file carving — all under a single tamper-evident, operator-signed audit ledger.

`Rust` · `Tauri v2` · `React` · `SQLite`

</div>

---

> **Disclaimer:** Purgent does not itself authorize any action. The operator is solely responsible
> for having the legal right to erase or analyze the target device/media.

---

## Why Purgent?

Conventional "shredders" and `dd`-style wipes operate at the host-OS layer — they cannot reach
wear-leveled NAND cells on modern SSDs, and they produce no evidence you can prove later. Purgent
closes that gap:

- **Hardware-direct sanitization** — issues ATA Secure Erase / NVMe Sanitize commands straight to
  the drive controller, then verifies by read-back.
- **Hidden-area removal** — detects and removes **Host Protected Areas (HPA)** and **Device
  Configuration Overlays (DCO)** so no hidden sectors survive your erase.
- **Tamper-evident evidence** — every operation produces a signed report (HMAC-SHA256) you can
  verify later, plus JSON / XML / PDF exports for audit workflows.
- **Air-gapped first** — works fully offline; sync to Supabase is an optional, opt-in mirror of
  audit *metadata* only.

---

## Features

| Capability | Details |
| --- | --- |
| 📀 **Targeted erasure** | Real device targets (ATA / NVMe) or image files, with operator confirmation before *any* destructive pass |
| 🔒 **Hardware secure erase** | ATA Secure Erase (SATA) and NVMe Sanitize (cryptographic / block erase) issued to the controller |
| 🛡️ **HPA / DCO removal** | ATA task-file commands (SET MAX ADDRESS / EXT, DCO RESET) issued before erase, with post-command re-query |
| 🧹 **Overwrite methods** | NIST Clear-style single-pass and multi-pass overwrite with pseudo-random + complement passes and read-back verification |
| 📄 **File eraser** | Per-file secure deletion incl. slack-space scrubbing (best-effort) |
| 🔍 **File carving & recovery** | Signature-based recovery, fragmentation reconstruction, confidence ranking, and hash verification |
| 🗂️ **Classification** | Auto-classifies carved files by category for triage |
| 🧾 **Signed reports** | Offline-first SQLite case DB, HMAC-SHA256 signatures, `verify_report`, JSON/XML/PDF export |
| 📶 **Optional sync** | Opt-in upload of report *metadata* to Supabase REST with retry + dedup (never file contents) |
| 🌐 **Cross-platform** | Windows (x64) primary, Linux (x64) via Ubuntu CI re-verified on every push |

> 📖 Full feature list → [`FEATURES.md`](FEATURES.md) · Step-by-step usage → [`USAGE_GUIDE.md`](USAGE_GUIDE.md)

---

## Standards compliance

| Standard | Type |
| --- | --- |
| **NIST SP 800-88 Rev.1 — Clear / Purge** | Built-in wipes + ATA Secure Erase / NVMe Sanitize paths |
| **DoD 5220.22-M** | Overwrite variants handled natively |
| **IEEE 2883-2022** | First-class purge semantics, surfaced in reports |
| **ISO/IEC 27037** | Evidence capture (pre-wipe SHA-256) & carving workflow |

---

## Architecture

```
┌─────────────────────────── src/ (React frontend)
│   Landing page · Tactical console · Report viewer
│
└─┬─ src-tauri/ (Tauri v2 shell — IPC bridge)
  │
  └─┬─ crates/purgent-core  (Rust engine, zero GUI deps)
    │
    ├─ storage/      drive enumeration, ATA/NVMe passthrough,
    │                HPA/DCO query + removal, secure erase
    ├─ drive_eraser  overwrite passes, verification, WipeResult
    ├─ file_eraser   file + slack scrubbing, per-file records
    ├─ recovery/     carving, fragmentation, classification
    ├─ reporting/    Report struct, signatures, JSON/XML/PDF export
    ├─ persistence/  offline-first SQLite case database
    ├─ signing/      operator key, identity fingerprint
    └─ sync/         optional Supabase metadata mirror (opt-in)
```

**Key security properties**

- Operator confirmation must exactly match the target before erasure begins.
- No **silent** fallback: if hardware erase is unavailable, the operator must explicitly
  acknowledge an overwrite fallback.
- Reports are signed locally with an operator-derived key and verifiable offline; sync never
  transmits recovered content.

---

## Tech stack

| Layer | Technology |
| --- | --- |
| Core engine | Rust (`serde`, `rusqlite`, `sha2`, `hmac`, `zeroize`) |
| Native shell | Tauri v2 (`windows-sys` / `libc` for raw device ioctls) |
| Frontend | React 18 + TypeScript, Vite, Tailwind CSS 4, lucide-react |
| Storage | SQLite (bundled) |
| Sync | Opt-in Supabase REST over `ureq` |

---

## Getting started

### Prerequisites

- **Node.js 20+** and npm
- **Rust 1.98+** (pinned via [`rust-toolchain.toml`](rust-toolchain.toml); `rustup` will pick it up automatically)
- **Windows:** MSVC C++ Build Tools ("Desktop development with C++") + WebView2 Runtime
- **Linux:** GTK / WebKit dependencies — see the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)

### Install & run

```sh
npm install
npm run tauri dev          # launch the desktop app (builds Rust + starts frontend)
```

### Frontend only (in a browser)

```sh
npm run dev                # Vite dev server → http://localhost:1420
```

> The browser version cannot issue device ioctls — those require the desktop shell.

### Build & test

```sh
npm run build              # typecheck + frontend production build
cargo build --workspace    # compile the Rust engine
cargo test --workspace     # full engine test suite (80+ tests)
npm run tauri build        # production desktop bundle
```

Rust version is pinned in [`rust-toolchain.toml`](rust-toolchain.toml) — `rustup` selects it
automatically; minimum supported Rust is 1.98.

### CLI demo (no UI)

```sh
cargo run --example wipe_demo     # exercise the core engine end-to-end on a temp image
```

---

## Development roadmap status

| Phase | Status |
| --- | --- |
| 1 – Foundation & overwrite engine | ✅ Complete |
| 2 – Cross-platform storage layer | ✅ Complete (Windows; Linux via CI) |
| 3 – Live-device wiping | ✅ Complete |
| 4 – File erase + slack scrub | ✅ Complete |
| 5 – Carving & recovery | ✅ Complete |
| 6 – Classification | ✅ Complete |
| 7 – Signed reporting | ✅ Complete |
| 8 – PDF / XML export | ✅ Complete |
| 9 – Tauri desktop app | ✅ Complete |
| 10 – Verification & hardening | ✅ Complete |
| 11 – Optional Supabase sync | ✅ Complete |
| 12 – HPA / DCO query **+ real removal** | ✅ Complete |
| 13–15 – Roadmap | 🔜 Planned |

*CI (`github/workflows/ci.yml`) re-verifies tests, formatting, and the frontend build on every push.*

---

## Documentation

| File | What it is (1 line) |
| --- | --- |
| [`README.md`](README.md) | Project overview, features, stack, setup, and roadmap. |
| [`USAGE_GUIDE.md`](USAGE_GUIDE.md) | Step-by-step instructions for every feature — wipe, file delete, recovery, reports, sync. |
| [`FEATURES.md`](FEATURES.md) | Complete, category-by-category feature inventory. |
| [`LEARNING.md`](LEARNING.md) | Hackathon prep — theory notes + judge-style Q&A. |
| [`Purgent_Project_Verification_Report.md`](Purgent_Project_Verification_Report.md) | External verification & gap-analysis report (v3). |
| [`Purgent_Frontend_TODO.md`](Purgent_Frontend_TODO.md) | Frontend code-review fix list (all items completed). |
| [`Purgent_Gaps_Action_Plan.md`](Purgent_Gaps_Action_Plan.md) | Gap-closure action plan (P0–P3) with live status. |
| [`KNOWN_ISSUES.md`](KNOWN_ISSUES.md) | Known correctness/security issues tracker (open + fixed). |
| [`HARDWARE_VALIDATION.md`](HARDWARE_VALIDATION.md) | Live hard-drive validation log (evidence required per RULES). |
| [`FORENSIC_VALIDATION.md`](FORENSIC_VALIDATION.md) | Recovery accuracy evidence against forensic datasets. |
| [`SECURITY_REVIEW.md`](SECURITY_REVIEW.md) | Security-hardening audit checklist + findings. |
| [`DECISIONS (4).md`](DECISIONS%20(4).md) | Engineering decision log (append-only). |
| [`PROJECT (4).md`](PROJECT%20(4).md) | Project plan and phase definitions. |
| [`AGENTS (4).md`](AGENTS%20(4).md) | Agent guidelines for working in this repo. |
| [`RULES (2).md`](RULES%20(2).md) | Hard safety and operating rules. |
| [`frontend-design-prompts.md`](frontend-design-prompts.md) | Frontend design scratch notes. |

---

## Repository layout

```
├── crates/purgent-core/     Rust engine (library + examples)
├── src/                     React frontend (components, assets, styles)
├── src-tauri/               Tauri v2 shell + IPC commands + icons
├── .github/workflows/       Ubuntu CI
├── USAGE_GUIDE.md / FEATURES.md / LEARNING.md
├── Purgent_Project_Verification_Report.md
├── DECISIONS (4).md         decision log
└── PROJECT (4).md / AGENTS (4).md / RULES (2).md
```

---

## License

See repository metadata. Purgent is provided for legitimate data-sanitization and forensic analysis
use; you are responsible for complying with all applicable laws before using it on any device.