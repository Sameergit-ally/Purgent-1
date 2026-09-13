# PROJECT.md — Purgent

Source of truth for what is being built. `AGENTS.md` codifies the build rules derived from this
document — read this first, then `AGENTS.md`, before writing any code. Project-wide usage,
ethical, and safety rules live in `RULES.md` and apply regardless of build phase.

---

## 1. What Purgent Is

Purgent is a unified, cross-platform desktop application that combines two capabilities normally
sold as separate tools:

1. **Secure Data Sanitization** — permanently, unrecoverably destroy data on drives or individual
   files, standards-compliant (NIST 800-88, DoD 5220.22-M, IEEE 2883-2022).
2. **Forensic-Grade Recovery** — recover deleted, corrupted, or formatted data from media via
   signature-based and structure-based file carving.

It targets digital forensics investigators, enterprise IT/security teams, government agencies, and
individuals who currently need two or more separate tools (e.g. DBAN/Eraser for wiping,
Autopsy/FTK/EnCase for recovery) to cover both needs.

The differentiator is not "it wipes" or "it recovers" individually — both exist elsewhere. It is:

- **One audit trail** covering both erase and recovery operations, so an investigator or auditor
  never has to reconcile logs from two different tools.
- **Standards-first design**, with every operation explicitly mapped to a recognized standard
  rather than a marketing claim of "secure delete."
- **Confidence-scored recovery** — every recovered file carries a transparency score instead of a
  black-box "recovered/not recovered" result.
- **Tamper-evident, signed reporting** suitable for chain-of-custody documentation.

---

## 2. Non-Negotiable Design Constraints

- **Disk-level operations require native OS access.** Secure erase (ATA Secure Erase, NVMe
  Sanitize, raw sector overwrite) and raw recovery scanning cannot run inside a browser sandbox.
  Purgent is a **desktop application** at its core; any web component is a companion viewer, not
  a substitute for the desktop engine.
- **No silent or automatic destructive action.** Every wipe operation requires explicit user
  confirmation naming the exact device (model, serial, capacity) before it starts. There is no
  "wipe all" or scheduled/unattended wipe path in this project.
- **Verification is mandatory, not optional.** Every erase operation must run a post-wipe
  read-back verification pass before it can be marked complete in the audit log. A wipe without a
  verification result is an incomplete operation, not a fast one.
- **Recovery never writes to the source media.** All recovery/carving operations mount source
  media read-only (write-blocked). Recovered output goes to a separate, user-chosen destination
  only.
- **Every operation (erase or recover) produces a signed, tamper-evident report.** `{ operation,
  device_or_file, standard_used, timestamps, operator_id, verification_result, report_hash }` is
  the minimum shape. An operation without a report is not considered complete.
- **`score_breakdown` (recovery) and `verification_result` (erase) are stored at operation time,
  never recomputed for display.** The dashboard must show the number actually produced by the
  operation, not a fresh approximation.
- **SSD handling is never blind-overwrite.** Wear-leveling makes overwrite-based wiping unreliable
  on flash media. SSD/NVMe devices must use ATA Secure Erase / NVMe Sanitize/Format commands, and
  the UI must disclose this distinction to the user before they proceed.

---

## 3. Architecture

### 3.1 Desktop Application (Primary Product)

- **Shell:** Electron (development speed, cross-platform) wrapping a native core, OR Tauri if the
  team wants a smaller footprint with a Rust-native shell — decide and record in `DECISIONS.md`
  before Phase 1 closes.
- **Core Engine:** Rust (preferred for memory safety in carving/wiping code) exposing a stable
  FFI/IPC boundary to the shell. C++ is acceptable only where a required low-level library has no
  mature Rust binding.
- **GUI Layer:** React (inside Electron/Tauri webview) or Qt widgets if a fully native UI is
  chosen — decide once, do not mix.

### 3.2 Core Engine Modules (Rust)

- **Storage/Device Abstraction Layer** — enumerates and identifies devices (HDD/SSD/USB/SD/NVMe),
  exposes a uniform interface over `libparted`/`hdparm`/`nvme-cli` (Linux) and Win32
  `DeviceIoControl` (Windows).
- **Secure Drive Eraser Module** — NIST 800-88 Clear/Purge, DoD 5220.22-M (3/7-pass), optional
  Gutmann; ATA Secure Erase / NVMe Sanitize for flash; HPA/DCO detection and removal; bad-sector
  skip+log; post-wipe read-back verification; signed certificate generation.
- **File/Folder Eraser Module** — multi-pass overwrite before unlink; metadata scrubbing (MFT
  entries, journal, thumbnails, recent-file lists, registry traces, `.bash_history`/Trash,
  Spotlight metadata); slack-space clearing; cross-filesystem support (NTFS, exFAT, FAT32, ext4,
  APFS); post-delete recovery-scan verification.
- **File Carving & Recovery Module** — signature-based carving (JPEG, PNG, PDF, DOCX/ZIP, MP4,
  and an extensible signature database); structure-based validation (JPEG segment markers, PDF
  xref tables); fragmented-file reconstruction (bifragment gap carving); confidence scoring;
  write-blocked read-only source mounting; SHA-256 hash of every recovered file at extraction time.
- **Hashing/Integrity Module** — SHA-256/SHA-3 for report signing, HMAC for tamper-evidence,
  shared by all other modules rather than reimplemented per-module.

### 3.3 Reporting & Audit Layer

- Generates signed, timestamped reports per operation: PDF (human-readable) and JSON/XML
  (machine-readable, for case-management integration).
- Local audit log is authoritative. Cloud sync (below) is a mirror, never the source of truth for
  an in-progress operation.

### 3.4 Data Storage

- **Local (authoritative):** SQLite, on the machine performing the operation. Forensic/case data
  must remain available and intact even with zero network connectivity — this is a hard
  requirement for air-gapped environments.
- **Cloud (optional, opt-in):** Supabase (PostgreSQL + Auth + REST API) for teams that need
  centralized, multi-location audit visibility. Sync is one-directional (local → cloud, append
  only) and must never be required for a wipe or recovery operation to complete locally.
- **Row-Level Security** in Supabase to separate access by role (operator, auditor, admin) when
  cloud sync is enabled.

### 3.5 Optional Companion Web Dashboard

- Read-only viewer (React/Next.js) connected to Supabase for remote audit-log/report viewing by
  non-operator stakeholders (e.g. a compliance manager).
- **Cannot initiate, control, or influence any wipe or recovery operation.** It has no write path
  to device state, ever.

---

## 4. Tech Stack

- **Core Engine:** Rust (primary), minimal C++ only where unavoidable
- **Desktop Shell:** Electron or Tauri (single choice, recorded in `DECISIONS.md`)
- **GUI:** React (web-view) or Qt (native) — single choice, recorded in `DECISIONS.md`
- **Local DB:** SQLite
- **Cloud DB (optional):** Supabase (PostgreSQL + Auth)
- **Hashing:** SHA-256 / SHA-3, HMAC
- **Reporting:** PDF generation library appropriate to chosen stack + structured JSON/XML export
- **Companion Web Dashboard (optional):** React/Next.js, read-only against Supabase
- **Packaging:** Platform-native installers (MSI/EXE for Windows, AppImage/deb for Linux, DMG for
  macOS if targeted)

---

## 5. Workflow

### 5.1 User Workflow — Erase

1. Select target: whole device or specific file/folder.
2. Tool auto-detects device type (HDD/SSD/USB/etc.) and shows applicable standards.
3. User explicitly confirms target identity (model/serial/capacity/path) before proceeding.
4. Operation runs with live progress; bad sectors logged, not fatal.
5. Post-wipe verification runs automatically.
6. Signed erasure certificate (PDF + JSON) generated; entry added to unified audit log.

### 5.2 User Workflow — Recovery

1. Select source media or disk image (mounted read-only).
2. Choose scan mode (signature-based / structure-validated / full raw scan).
3. Carving engine runs; results list shows recovered files with confidence scores.
4. User selects files to export to a separate destination.
5. Each exported file is hashed at extraction time; forensic report generated.
6. Entry added to the same unified audit log as erase operations.

### 5.3 Build Workflow

Follow the phase order in `AGENTS.md` exactly. Each phase has a hard checkpoint that must be
verified and reported before the next phase starts. Record any non-trivial technical or product
decision in `DECISIONS.md` at the time it's made. All builders and contributors additionally
follow `RULES.md`.

---

## 6. Standards & Compliance Mapping

| Standard | Applies To | Notes |
| --- | --- | --- |
| NIST SP 800-88 Rev.1 | Drive erasure | Clear / Purge / Destroy categories |
| DoD 5220.22-M | Drive/file erasure | Legacy but widely referenced |
| IEEE 2883-2022 | Drive erasure | Modern replacement addressing SSD/flash |
| ISO/IEC 27001 | Overall data handling/audit | Organizational compliance angle |
| ACPO Principles / ISO/IEC 27037 | Recovery module | Evidence handling, chain of custody |

A "Compliance Matrix" view (which module satisfies which standard) must be a first-class, always
up-to-date screen in the dashboard, not a document that goes stale after Phase 1.

---

## 7. Scope Discipline

**Core loop that defines the product (never cut):** device/file selection → standards-appropriate
erase or read-only recovery scan → verification/confidence scoring → signed report → unified audit
log entry.

**Cut order if scope needs to shrink (in order):**
1. macOS support (keep Windows + Linux)
2. Companion web dashboard
3. Cloud (Supabase) sync — local SQLite + local reports remain fully functional without it
4. Gutmann 35-pass legacy mode (NIST/DoD coverage is sufficient)
5. ML-based recovered-file classification (rule-based classification remains)

**Never cut:** post-operation verification, signed/tamper-evident reporting, read-only source
mounting for recovery, and the unified audit log across both erase and recovery.

---

## 8. Reference

Original concept/market playbook: `Secure_Erasure_Recovery_Tool_Playbook.pdf`.
Build rules and phase checkpoints for the coding agent: `AGENTS.md`.
Usage, ethical, legal, and safety rules for the project: `RULES.md`.
Running log of technical/product decisions: `DECISIONS.md`.
