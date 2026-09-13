# DECISIONS.md — Purgent Decision Log

A running, append-only log of non-trivial technical and product decisions made while building
Purgent. See `AGENTS.md` §6 for the rule governing this file.

**How to use this file (for the coding agent):**
- Add a new entry **at the time a decision is made**, not retroactively.
- Never edit or delete a past entry. If a decision is later reversed or changed, add a new entry
  that references the old one and explains why.
- Keep entries short: what was decided, why, and what the alternative would have been.
- Order entries chronologically, oldest first.

**Entry format:**

```
## [YYYY-MM-DD] Short title of the decision

**Phase:** which build phase this happened in
**Decision:** what was decided
**Why:** the reasoning
**Alternative considered:** what else was on the table, and why it was passed over
```

---

## [2026-09-13] Autonomous phase progression

**Phase:** 1
**Decision:** The coding agent proceeds through the 15 build phases continuously, without asking
the user for permission to start each phase. After finishing a phase it runs the checkpoint
verification and test/lint/build commands, fixes any defects found, updates repo docs/status, and
immediately starts the next phase.
**Why:** The user (project owner) requested this explicitly so the build can advance without
back-and-forth; the hard gates are preserved by requiring the checkpoint to be genuinely verified
before the next phase begins.
**Alternative considered:** Stopping for approval after every phase — rejected as too slow; the
per-phase gates already enforce quality on their own.

## [2026-09-13] Desktop shell: Tauri (v2) + React

**Phase:** 1
**Decision:** Tauri v2 with a React frontend, in a Rust workspace where the core engine lives in a
separate `purgent-core` crate (no Tauri dependency) and the Tauri app crate (`src-tauri`) exposes
core functions to the UI via Tauri IPC commands.
**Why:** Tauri's native Rust shell matches the Rust core engine (one language across the whole
native stack), the resulting binary is far smaller and leaner than Electron, and its default
security posture (capabilities/permission system, strict CSP, no Node in production) fits a
forensic/security tool. Keeping the engine in its own crate makes the wipe and recovery modules
independently testable with plain `cargo test`, which the build rules require anyway.
**Alternative considered:** Electron + React — fastest iteration and largest ecosystem, but a ~150MB+
memory-heavy runtime and a Node backend that does not align with the Rust-native architecture.
Qt Widgets native UI was considered but rejected because our team is more productive in a web-based
UI and Tauri provides the smaller native shell.

## [2026-09-13] Phase 3 wipe semantics: verification stored at operation time, tests on image files

**Phase:** 3
**Decision:** The drive eraser (NIST SP 800-88 Rev.1 Clear/Purge and DoD 5220.22-M) records its
mandatory post-wipe read-back verification as a snapshot inside the operation result at wipe time
and never recomputes it later. Confirmation of a wipe requires the operator to type back the exact
target description; a mismatch refuses the operation. All wipe testing targets image files only.
**Why:** Recording verification at operation time closes the "recompute on demand" loophole that
could let a later report re-verify against an already-presented target and claim a pass that was
never actually checked; exact-string confirmation follows the project rule that auto-confirmation
and bypass flags are forbidden. Image-file targets keep development safe (no production media).
**Alternative considered:** Deriving verification lazily at report time — rejected (unsound audit
trail). A check box that is pre-checked by default — rejected as an auto-confirmation bypass.

## [2026-09-13] Phase 4 file eraser hardening: exclusive-open refusal and delete-after-verify

**Phase:** 4
**Decision:** The file/folder eraser opens every target file in exclusive mode (zero sharing on
Windows) so a double-open by any other process fails the wipe and reports "in use" instead of
proceeding. Files are deleted only after the post-erase read-back verification passes; a file whose
verification fails is left in place and reported as incomplete.
**Why:** The project rule requires the tool to refuse graceful wipes of in-use files, and requires
erasure to be incomplete until blocks are verified gone — deleting unverified content would let a
"forensic" deletion hide data instead of erasing it.
**Alternative considered:** Force-closing other handles or deleting without verification — both
rejected as violating the no-bypass and verify-before-complete rules.

## [2026-09-13] Phase 5 carving: signature database in config, read-only source, hashes at extraction

**Phase:** 5
**Decision:** Signature-based carving lives behind an extensible signature database returned by
`config::signature_database()` (one entry per file type: header magic, how to find the end of the
file, max carve size). The source media/image is opened read-only. Every carved file is hashed with
SHA-256 at the moment its bytes are written to the output file, and that hash is stored in the
recovery report. Phase 5 ships six signature types (JPEG, PNG, GIF, BMP, PDF, ZIP).
**Why:** The build rules require the signature database to be centralized and extensible rather than
hardcoded inline (easy to add file types without touching the scanner). Read-only opening enforces
the write-blocking rule at the API level, and hashing at extraction time is what lets the carve
checkpoint prove recovered bytes match known-good hashes.
**Alternative considered:** Hardcoding magics inside the scanner; carving without recording hashes;
embedding each file's size guess in the signature registry. All rejected — the first two violate
explicit build rules, the third would spread format knowledge out of the config layer. Structure
validation, fragmented-file reconstruction, and MP4/DOCX signatures are explicitly deferred to
Phase 6.

## [2026-09-13] Phase 6 recovery: structure validation, confidence scoring, JPEG bifragment gap carving

**Phase:** 6
**Decision:** Every carved file is structurally validated by its own file type (JPEG segment walk to
EOI — raw entropy after SOS is allowed, PNG chunk walk with CRC-32, GIF header+trailer check, BMP
declared-size/dib-offset sanity, PDF magic/object/EOF presence, ZIP local-header + EOCD check). Each
recovery is ranked with an explicit `confidence` score plus a per-factor breakdown
(`structure_valid`, `terminator_present`, `fragment_reconstructed`, `size_consistent`): invalid
structures score 0.30, valid fragmented reconstructions 0.74, valid contiguous files 0.95 / 0.84.
Bifragment gap carving (bifragment-gap-carving) reconstructs a JPEG split at a segment boundary with
a gap in between: the first fragment ends at the last parseable segment, the second begins at the
next valid segment marker, the gap bytes are dropped, and only the rebuilt bytes are hashed and
written. The declared `size_bytes` is the recovered content length; the source extent (including the
dropped gap) is derivable from `source_offset` + `gap_bytes`.
**Why:** Hash-only carving proves integrity of whatever was carved but cannot tell recoverable files
apart from artifacts; structure validation gives the operator an evidence-grade signal and ranking,
and it is what the tests verify (known-good ranks above known-bad, fragmented JPEG rebuilt byte-for-
byte from a 64 MB gap matches the original hash).
**Alternative considered:** Validating only via hash comparison against a pre-built corpus — rejected
because the tool must judge unknown evidence, not only test fixtures. File-existence/repair libraries
per format were considered and rejected to keep the core engine dependency-light and self-contained.
MP4/DOCX structural validation remains deferred to a later phase (signatures are not yet registered).

## [2026-09-13] Phase 7 signed reports: HMAC-SHA256 over canonical payload, per-install keystore

**Phase:** 7
**Decision:** Every operation (wipe, file erase, carve) is produced through a single `operations`
facade that runs the module logic, builds a report, signs it, and persists JSON + PDF certificate.
Each report carries a `report_hash` (SHA-256 of the canonical payload) and an `signature` (HMAC-SHA256
of the canonical payload) with an explicit `signature_alg` field; the canonical payload drops the
three audit fields (`report_hash`, `signature_alg`, `signature`) and is re-serialized through a
sorted-key JSON map so the signed bytes are stable. Verification recomputes both the hash and the
HMAC from the on-disk JSON and rejects any mismatch, any wrong key, or an unexpected
`signature_alg`. The signing key is a random 32-byte file created once per install in the user
config/app-data directory (`%APPDATA%/Purgent/signing.key`, `~/.config/purgent/signing.key`); it is
never committed, logged, or stored beside the repo, and report verification takes the key as input.
**Why:** A plain SHA-256 in the report detects accidental edits but anyone can recompute it and launder
the report; HMAC-SHA256 ties the report to the install key so a doctored report cannot be re-signed
without the key file, and the sorted-key canonical serialization keeps the verifier independent of
struct field order. Keeping the key out of the repository satisfies the no-secrets-in-source rule
while still allowing offline per-install verification.
**Alternative considered:** Asymmetric signatures (Ed25519) — stronger non-repudiation but requires
key-management/tooling now; deferred until cloud sync (Phase 11) makes a key-hierarchy decision
meaningful. Bundling a hardcoded shared secret was rejected outright (secret would ship to every
install). Building only the wipe report was considered, but the checkpoint explicitly covers
operations from Phases 3–6, so file-erase and carve reports were made first-class too.

## Phase 8 — GUI Dashboard (Drive/Task Selector, Progress Monitor, Report Viewer, Compliance Matrix)

**Decision:** Progress state and completion are exposed to the React dashboard as Tauri IPC commands
plus webview events. Core erasure/recovery functions gained `*_with_progress` variants that accept a
`&mut dyn FnMut(ProgressUpdate)` callback (`progress.rs`, values: operation_id/type/phase,
bytes_done/total_bytes/message with a `fraction()` helper); the original public functions delegate
with a no-op callback so existing tests and callers are unchanged. The Tauri layer runs each
operation on a spawned thread (`wipe_image`, `wipe_device`, `erase_path`, `carve_source`), streaming
`progress` events and terminating with `operation-complete` (report paths) or `operation-error`.
The dashboard provides an exact-string confirmation modal whose required phrase is derived from
`WipeTarget::display()`/`EraseTarget::display()`, so the GUI cannot bypass the RULES confirmation
gate; a mismatched phrase is refused by the core. Operator identity is a small first-run input
persisted to `operator.txt` beside the signing key (defaults to the OS username); the report
directory is Tauri `app_data_dir/reports`. `list_reports` revalidates every stored JSON report
against the install key and surfaces a per-row Verified/INVALID badge plus a compliance matrix
(verification status, mismatch count, signature validity, report hash); report JSON is viewable and
the PDF certificate opens via the OS default viewer. Device wiping is allowed only for targets whose
path contains `PHYSICALDRIVE` as a conservative guard.
**Why:** A clickable full workflow requires live progress and a way to audit results in-app rather
than reading files on disk. Callback-based progress in core keeps the engine GUI-free while still
being unit-testable; emitting events instead of blocking invoke calls keeps the UI responsive for
multi-GB targets. Listing with revalidation makes tamper-evidence visible at a glance, which is the
Phase 8 checkpoint requirement.
**Alternative considered:** Exposing progress as a shared atomic state polled by the UI (would add
polling latency and lifecycle cleanup for no benefit on small workloads). Blocking multi-GB wipes on
the IPC thread was rejected (freezes the UI, and Tauri commands are not async-friendly for CPU/long
work here). Editing the persisted JS API beyond a `set_operator` call was avoided; identity remains
server-side (core credential, not frontend).

## Phase 9 — Local persistence (SQLite)

**Decision:** A `persistence.rs` module in the core engine owns a `CaseDb` over SQLite (rusqlite,
`bundled`, WAL journaling) stored at `app_data_dir/reports/case.sqlite3`. The DB mirrors every
completed report: full canonical JSON text plus indexed columns (report/operation ids, type,
operator, target, standard, timestamps, capacity, verification status, mismatch/skipped counts,
report hash, signature alg + signature, and the on-disk JSON/PDF paths). New operations are written
to the file store by the existing `save_report` and additionally recorded into the DB by the Tauri
thread right before the `operation-complete` event. `list_reports`, `read_report`, and
`open_report_pdf` now read through the DB (lookup by report id, PDF path resolved from the DB),
keeping the UI working fully offline. On startup the app runs `import_directory` to backfill any
legacy `report-*.json` files present in the reports folder into the DB (idempotent by report id),
so a pre-existing audit trail survives this upgrade.
**Why:** The Phase 9 checkpoint demands audit data survive a mid-session kill and relaunch with
zero network access. Files alone would satisfy that, but a SQLite index gives the Compliance Matrix
and Report Viewer a stable, queryable source (ordering, filtering later, idempotent dedup) rather
than rescraping a folder every time, and it captures metadata (verification status, signature,
hash) as first-class columns for future reporting. Verifying tamper-evidence still always
recomputes the HMAC from the stored JSON — the DB never stores a trusted "verified" flag — so the
Phase 7 guarantees are preserved.
**Alternative considered:** Scanning the reports directory for `list_reports` (no DB) was the
status quo but offers no index, no dedup, and no metadata querying; rejected for Phase 9 since the
app already keeps files too and the DB adds cheap resilience. Storing reports only in SQLite without
files was rejected because the JSON + PDF walk-away artifacts are part of the deliverable and the
PDF certificate is generated at operation time. Using `sqlx`/async was rejected — the engine is
synchronous and `rusqlite` bundles SQLite so there is nothing external to install at runtime.

## Phase 10 — Cross-platform expansion (second target OS: Linux)

**Decision:** Linux (x64) is the second target OS. Linux device enumeration was added as
`storage/linux.rs` reading `/sys/class/block` (whole-disk filtering via the `partition` attribute
plus name heuristics for `sd`/`vd`/`hd`/`nvme`/`mmcblk`/`sr`; capacity from the `size` attribute;
removable/media/bus classification), replacing the empty-vector stub on non-Windows. Live-device
wiping gained `open_device_linux` (`/dev/sdX`, `/dev/nvmeXnY`, …) opened read/write, root-gated, and
`WipeTarget::Device` now dispatches on `cfg(windows)` / `cfg(target_os = "linux")`. The Tauri
`wipe_device` guard was generalized from "must contain PHYSICALDRIVE" to a cross-platform block-device
allow-list (`\\.\PHYSICALDRIVE*`, `/dev/sd*`, `/dev/vd*`, `/dev/hd*`, `/dev/nvme*`, `/dev/mmcblk*`,
`/dev/sr*`, `/dev/loop*`). A `.github/workflows/ci.yml` runs `cargo check`/`cargo test`/`cargo fmt
-- --check` on `purgent-core` plus `npm ci && npm run build` on Ubuntu for every push/PR, which
continuously re-verifies the Phase 3–8 checkpoints on the second OS.
**Why:** The phase's checkpoint is "all Phase 3–8 checkpoints re-verified on the second OS". The
image-file based engine tests (wipe, file erase, carve, signing, reporting, persistence) are
platform-independent and were already in the suite — running them on Ubuntu via CI is the reachable,
reproducible verification, while a real loop-device wipe needs root on a Linux host and is explicitly
outside an unattended CI run. Keeping the block-device open behind `cfg` keeps Windows the primary
and Linux a first-class target without a second UI framework (Phase 1 decision holds for both).
**Alternative considered:** Adding all `#[cfg(not(windows))]` code paths under a single "unix"
umbrella was rejected — macOS stays out of scope (see PROJECT.md §7), and `target_os = "linux"`
keeps a later macOS port from silently double-enabling Linux sysfs code. Using udisks/D-Bus on
Linux for enumeration was rejected: `sysfs` is lighter, has no runtime daemon dependency, and is
what the kernel itself publishes; desktop notification of hotplug events is Phase 12+ territory.

## Phase 11 — Supabase cloud sync (optional layer) + Auth + RLS

**Decision:** Sync is an opt-in, strictly offline-first layer. A new `sync.rs` module defines
`SyncConfig::from_env()` (reads `PURGENT_SUPABASE_URL` and `PURGENT_SUPABASE_ANON_KEY`, never
committed or logged), a `SyncTransport` trait (production `HttpTransport` uses `ureq`; tests inject
a mock), and `AuditSyncClient` which POSTs one *audit-metadata row* per report to
`{url}/rest/v1/audit_logs` (report/operation ids, type, operator, target, standard,
capacity/timestamps, verification status + detail, mismatched sectors, report hash, signature alg +
HMAC). Recovered *file contents* are excluded by construction (only the report row travels; carving
outputs stay strictly on local disk). Each completed operation is recorded to the local SQLite
`CaseDb` first (`record_report`) and only then does a detached thread run `sync_unpushed`, which
uploads every report not already marked in the new `sync_state` table, marking each on success and
leaving failures queued for the next pass. The UI shows a "Cloud sync" panel (En/disable + Sync
now + counts) and the app honors an `sync_optin.txt` flag so sync defaults to off.
**Why:** The checkpoint is "disable the network mid-operation, confirm the local operation still
completes and reports normally". Routing every completion through a best-effort, mock/ureq-verified
sync pass after local persistence guarantees that contract structurally — the wipe/erase/carve
finishes and its signed report is viewable/verifiable even with an unreachable endpoint or missing
env config. Opt-in + env-gated credentials + metadata-only payloads satisfy the RULES (no auto-
exfil; secrets never committed). The `sync_state` table gives idempotent, retryable mirroring with
no reliance on Supabase filtering for correctness (RLS is still recommended to scope `audit_logs`
to the operator's org in Phase 11's deployment docs).
**Alternative considered:** Blocking the operation-complete event on a successful upload was
rejected immediately — that inverts the offline-first requirement and couples local audit to cloud
health. Using `supabase-rs`/`reqwest` was rejected: both are heavier; `ureq` is synchronous,
matches the engine, and TLS via rustls keeps install simple. Full blob/media (blob storage of
carved files) was excluded — it is out of the audit-only scope and would risk bulk exfil of
recovered evidence, contradicting RULES. Real-endpoint end-to-end verification is exercised
manually with the user's Supabase project; the automated suite validates the behavior contract
against a mock transport and an unreachable endpoint.
