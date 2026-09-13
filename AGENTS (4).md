# AGENTS.md — System Instructions & Rules

Execution rules for any LLM agent (OpenCode, Claude Code, Cursor, etc.) working in this repository.

**Project:** Purgent — a unified desktop application for standards-compliant secure data erasure
and forensic-grade file recovery, with a single tamper-evident audit trail across both.
**Source of truth:** `PROJECT.md` (read it before building; this file codifies its rules for agent
behavior). Project usage/ethics/safety rules that apply regardless of build phase: `RULES.md`.
Original concept document: `Secure_Erasure_Recovery_Tool_Playbook.pdf`.

This is a **full production build**, not a hackathon MVP. Phases assume real hardening, real
compliance validation, and real testing — do not compress phases to "just make it work."

---

## 1. Build Discipline — The 15 Phases

Work through the phases **IN ORDER**. After each phase, run/verify its checkpoint and report the
result **before** starting the next phase. Do not skip ahead, do not parallelize phases.

| Phase | Goal | Checkpoint (hard gate) |
| --- | --- | --- |
| 1 | Project setup & architecture finalization (Rust core skeleton, Electron/Tauri shell choice recorded, repo/Git, README skeleton, CI scaffold) | Shell boots to a blank window; Rust core builds and passes a hello-world FFI/IPC call from the shell |
| 2 | Storage/Device Abstraction Layer (enumerate HDD/SSD/USB/NVMe, cross-platform) | Tool lists all connected devices with correct model/serial/capacity/media-type on both target OSes |
| 3 | Secure Drive Eraser Module (NIST 800-88, DoD 5220.22-M, ATA Secure Erase/NVMe Sanitize) | Wipe a test virtual/loopback device with each supported standard; post-wipe read-back verification passes; certificate (PDF+JSON) generated and hash-valid |
| 4 | File/Folder Eraser Module (multi-pass overwrite, metadata scrubbing, slack-space clearing) | Erase a test file/folder tree; independent recovery-scan attempt on the same location finds nothing recoverable |
| 5 | File Carving & Recovery Module — signature-based (5–10 file types) | Carve a prepared test image; recovered files match known-good hashes for at least the target file types |
| 6 | File Carving & Recovery Module — structure validation + fragmented-file reconstruction + confidence scoring | Confidence scores correctly rank a known-good/known-bad mixed test set; a deliberately fragmented file is reconstructed correctly at least once |
| 7 | Hashing/Integrity + signed, tamper-evident Reporting & Audit system | Every operation from Phases 3–6 produces a report whose hash verification fails if the report is edited post-generation |
| 8 | GUI Dashboard integration — Drive/Task Selector, Progress Monitor, Report Viewer, Compliance Matrix | Full user workflow (select device/file → operate → verify → view report) clickable end-to-end in the desktop app |
| 9 | Local persistence (SQLite) — case/audit data survives restarts, works fully offline | Kill the app mid-session, relaunch, confirm all prior operations and reports are intact with zero network access |
| 10 | Cross-platform expansion (second target OS) | All Phase 3–8 checkpoints re-verified on the second OS |
| 11 | Supabase cloud sync (optional layer) + Auth + RLS | Enable sync: local operations mirror to Supabase; disable network mid-operation, confirm local operation still completes and reports normally |
| 12 | Companion web dashboard (optional, read-only) | Dashboard displays synced audit data live; confirm it has no code path that can trigger, pause, or alter a device operation |
| 13 | Security hardening & adversarial testing (privilege boundaries, write-blocking on recovery targets, input validation on all device/path selectors) | A written test log showing each hardening check attempted and its result; no unresolved high-severity finding remains open |
| 14 | Full QA pass — validation datasets (e.g. NIST CFReDS test images), regression suite, standards compliance sign-off against Section 6 of `PROJECT.md` | Full erase+recovery+report loop run 10× consecutively across both OSes with no crash, no lost/duplicated audit entries, no compliance-matrix mismatch |
| 15 | Packaging, documentation, and release (native installers, user manual, technical/architecture docs, performance report) | Fresh machine (no dev tools installed) can install from the packaged artifact and complete one erase and one recovery operation end-to-end |

**Never cut:** device/file selection → standards-appropriate operation → mandatory verification →
signed report → unified audit log entry, with read-only source mounting for all recovery
operations. This loop is the entire product; everything else in `PROJECT.md` §7's cut order is
genuinely optional.

**Autonomous progression:** The agent works through the phases continuously and does NOT ask the
user for permission to start a phase. When a phase's work is complete, the agent runs the phase's
checkpoint verification plus the project's test/lint/build commands, fixes any defects those turn
up, updates repo docs and status (`README.md`, `DECISIONS.md`, `PROJECT.md` as appropriate), and
then starts the next phase without pausing for approval. The agent only stops to ask the user when
facing a genuine blocker or an architectural decision that has not already been recorded in
`DECISIONS.md`.

## 2. Forbidden Actions

- **Never mark an erase operation complete without a passed post-wipe verification.** A wipe
  without verification is an incomplete feature, not a faster one.
- **Never allow a wipe to start without the user explicitly confirming the exact target**
  (model/serial/capacity or full file path) shown back to them in the confirmation step. No
  "wipe last selected device" shortcuts, no default-selected device in any wipe dialog.
- **Never perform blind multi-pass overwrite on SSD/NVMe media as the primary method.** Use ATA
  Secure Erase / NVMe Sanitize/Format and disclose the distinction in the UI; overwrite-only mode
  on flash media requires an explicit user acknowledgment of reduced reliability.
- **Never mount source media read-write during a recovery/carving operation.** Write-blocking is
  mandatory, not configurable off.
- **Never recompute `score_breakdown` or `verification_result` for display.** Store the value
  produced at operation time and read it back.
- **Never generate a recovered file or erasure certificate without a SHA-256 hash captured at the
  moment of creation.**
- **Never call real cloud publishing/exfiltration paths beyond the explicitly scoped Supabase
  sync.** No feature may transmit recovered file *contents* off the local machine automatically;
  only audit metadata (operation type, device info, hashes, timestamps, scores) syncs to Supabase,
  and only when the user has opted in.
- **Do not skip or reorder phases** to reach later features. Stretch/optional layers (Supabase
  sync, companion web dashboard, macOS support) only start after their preceding phase is
  checkpointed.
- **Do not commit, push, or create PRs** unless the user explicitly asks.
- **Never hardcode or log secrets/API keys** (Supabase keys, signing keys, any credentials) in
  code, logs, README, or docs. Keep them out of source control (`.gitignore`).
- **Never reuse or vendor Sleuth Kit (or other GPL/AGPL forensic tool) source directly.** Use such
  tools only as a behavioral reference; all carving/parsing logic in this repo must be original
  implementation to avoid licensing conflicts, as specified in `PROJECT.md`.
- **Never add an auto-wipe, scheduled-wipe, or unattended-wipe path.** Every destructive operation
  requires a live, explicit user action in the current session.

## 3. Code Style

- **Core engine:** Rust, safe-by-default; `unsafe` blocks require a comment explaining why no
  safe alternative exists (this is the one place comments are required — everywhere else, prefer
  self-documenting code and clear names over comments).
- **Shell/GUI:** match whatever stack is recorded in `DECISIONS.md` for Phase 1 (Electron+React or
  Tauri+React or native Qt) — do not introduce a second UI framework later.
- **Modules stay separated.** Storage Abstraction, Drive Eraser, File/Folder Eraser, Carving &
  Recovery, Hashing/Integrity, Reporting/Audit are distinct modules with a defined interface
  between them — do not collapse any two together. This separation is what keeps the wipe and
  recovery code paths independently auditable, which is the entire compliance pitch.
- **No code comments unless required by the `unsafe` rule above or explicitly asked.**
- **Log every state transition** for both erase and recovery paths: `selected → running →
  verifying → verified/failed → reported`. Use a consistent prefix (e.g. `[purgent]`). This is what
  makes live debugging and any live demo/audit review fast.
- **Config to centralize** (in a single config module):
  - Supported wipe standards and their pass counts/patterns
  - Device-type detection thresholds/rules
  - Signature database location for carving (extensible, not hardcoded inline)
  - Supabase connection details (loaded from environment, never committed)
  - Default report export paths and formats
- Match existing conventions when editing; don't introduce new patterns for the sake of it.

## 4. Terminal / Testing Policy

- Verify each phase checkpoint with the prescribed test before reporting completion (see table in
  §1). Use virtual disks / loopback devices / disk image files for all destructive testing —
  **never test wipe operations against a real production drive** without an explicit, separate
  user instruction to do so.
- Run the project's typecheck/lint/build command after any code change; fix all errors before
  moving on.
- Isolate module testing: verify the Scoring/confidence logic and the Erase verification logic
  against hand-checked expected values *before* wiring the GUI on top of either, so a core-logic
  bug is never mistaken for a UI bug.
- Recovery module validation must include at least one recognized public forensic test dataset
  (e.g. NIST CFReDS images) before Phase 14 can be marked complete.
- Any flaky or platform-specific low-level API failure (device enumeration, ATA/NVMe command
  support) should be logged and inspected before being treated as a code defect — some behavior is
  genuinely hardware/driver-dependent.

## 5. Documentation

- Keep `README.md` current with build/run instructions, supported platforms, and current phase
  status.
- Do not create documentation files unless asked; update the ones specified in the build plan
  (`README.md`, `PROJECT.md`, `DECISIONS.md`, `RULES.md`).
- Keep `PROJECT.md` and `DECISIONS.md` in sync with what is actually built; only mark a phase
  checkpoint complete in status updates when it is genuinely verified per §4.
- User manual, technical/architecture documentation, and the performance evaluation report
  (Phase 15) are deliverables, not optional polish — they are part of the hard gate for Phase 15.

## 6. Decision Log

- Record every non-trivial technical or product decision in `DECISIONS.md` **at the time it's
  made**, not retroactively at the end of a session.
- A decision is "non-trivial" if a different reasonable choice existed and someone reviewing the
  repo later would benefit from knowing why this one was picked (e.g. Electron vs. Tauri, choice
  of wipe-pass pattern, why a field was added to the audit schema, why a phase's scope was
  narrowed).
- Follow the format already established in `DECISIONS.md` — one entry per decision, in
  chronological order, never edited or deleted after the fact (add a follow-up entry instead if a
  decision is later reversed).
