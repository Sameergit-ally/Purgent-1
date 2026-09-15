# Purgent — Project Verification & Gap Analysis Report

**Compared against:** *Secure Erasure & Recovery Tool Playbook* (original concept doc)
**Codebase reviewed:** `Purgent-1-main` (Tauri v2 + Rust core `purgent-core` + React frontend)
**Date:** September 2026

---

## 1. Executive Summary

Purgent is **not** a hackathon-level mockup — it's a genuinely engineered, phase-gated build with a
clean architecture (Rust core engine separated from the Tauri shell), a real decision log
(`DECISIONS.md`), enforced project rules (`RULES.md`), and 44 passing unit tests. Phases 1–11 (of a
planned 15) are marked complete, and the code substantiates most of those claims.

However, several features that the **original playbook treats as core** — and that the project's
own `RULES.md`/`AGENTS.md` explicitly mandate — are **not yet implemented in the backend**,
most notably SSD-safe erasure (ATA Secure Erase / NVMe Sanitize) and file-level metadata scrubbing.
These are flagged below as the highest-priority gaps.

**Verdict:** Strong foundation, real production discipline, but the "hard" forensic/hardware-level
features are still ahead — not the GUI/reporting/audit layer, which is comparatively very mature.

---

## 2. What's Actually Done (Verified in Code)

| Playbook Module | Status | Evidence |
|---|---|---|
| Architecture (unified platform, Rust core, cross-platform shell) | ✅ Done | Tauri v2 + React; `purgent-core` crate is GUI-independent and separately testable |
| Secure Drive Eraser — NIST 800-88 (Clear/Purge) | ✅ Done | `drive_eraser.rs`, `config.rs` — implemented, tested |
| Secure Drive Eraser — DoD 5220.22-M (3-pass) | ✅ Done | Zero → 0xFF → seeded-random passes, verified |
| Post-wipe read-back verification | ✅ Done | Mandatory; stored at operation time, never recomputed (closes a real audit-integrity loophole) |
| Bad sector handling (skip + log) | ✅ Done | `skipped_sectors` tracked and reported, doesn't fail the whole wipe |
| Exact-target confirmation before wipe | ✅ Done | Operator must type back model/serial/path; mismatch refuses the operation |
| File/Folder Eraser (content-level) | ✅ Done (partial — see gaps) | Multi-pass overwrite → verify → delete-only-if-verified; exclusive-open refusal for in-use files |
| File Carving — signature-based | ✅ Done (6 of 10+ types) | JPEG, PNG, GIF, BMP, PDF, ZIP — extensible signature DB in `config.rs` |
| File Carving — structure validation | ✅ Done | JPEG segment walk, PNG CRC-32 chunk walk, ZIP EOCD check, PDF object/EOF presence, etc. |
| Fragmented file reconstruction | ✅ Done | Bifragment gap carving implemented and tested for JPEG |
| Confidence scoring | ✅ Done | Per-factor breakdown: structure validity, terminator presence, fragment reconstruction, size consistency |
| Read-only / write-blocked recovery source | ✅ Done | Enforced at the API level, not just a UI toggle |
| SHA-256 hashing at extraction time | ✅ Done | Every recovered file and every report is hashed on creation |
| Tamper-evident signed reporting | ✅ Done | HMAC-SHA256 over canonical (sorted-key) JSON payload; per-install signing key never committed |
| Report export | ✅ Partial | PDF (human-readable) + JSON (machine-readable) — **XML missing** |
| GUI Dashboard | ✅ Done | Drive/task selector, live progress via Tauri IPC events, report viewer, Compliance Matrix screen |
| Compliance Matrix (first-class UI screen) | ✅ Done | `App.tsx` renders a live matrix, not a static slide |
| Local persistence (SQLite) | ✅ Done | `persistence.rs`, survives app kill + relaunch, fully offline |
| Cross-platform (2nd OS) | ✅ Done | Linux device enumeration via `/sys/class/block`; CI runs full suite on Ubuntu |
| Optional cloud sync (Supabase) | ✅ Done | Opt-in, env-gated, metadata-only (no file contents), offline-first, retry/dedup logic |
| Operator identity logging | ✅ Done | Persisted per install, attached to every operation |
| Anti-bypass rules (no scheduled/unattended wipe, no auto-confirm) | ✅ Done | Enforced structurally, not just documented |

---

## 3. What's Missing — Prioritized Gaps

### 🔴 P0 — Critical (violates the project's own stated rules)

**1. SSD-safe erasure is not implemented at all.**
`RULES.md` explicitly states the tool "must never silently fall back from a hardware-backed method
(ATA Secure Erase / NVMe Sanitize) to blind overwrite on flash media." Right now there **is no ATA
Secure Erase / NVMe Sanitize/Format code path** — only overwrite-pass wiping exists (NIST/DoD
patterns). This means on real SSD/NVMe hardware, current wipes are **not standards-reliable** the
way the README claims. This is the single biggest gap between "what's documented as done" and
"what the code actually does."
> Fix: Implement `hdparm`-style ATA Secure Erase (Linux) and `DeviceIoControl`/NVMe pass-through
> (Windows) behind the Storage Abstraction Layer; require explicit operator acknowledgment when
> falling back to overwrite, per the tool's own rule.

**2. File/Folder Eraser has no metadata scrubbing.**
Playbook §4.2 (and `PROJECT.md` §3.2) require scrubbing of: filenames in MFT/journal entries,
thumbnail caches, recent-file lists, Windows registry traces, `.bash_history`/Trash (Linux),
Spotlight metadata (macOS), and slack-space clearing at the file's previous cluster location. The
current `file_eraser.rs` only overwrites file *content* and deletes the file — none of this
metadata-layer scrubbing exists yet.
> Fix: Add an OS-specific "trace scrubber" sub-module (behind the same abstraction pattern as
> storage) that runs after content wipe: registry key cleanup (Windows), shell history/trash
> purge (Linux), thumbnail cache purge, and slack-space overwrite of the file's last allocated
> clusters.

### 🟠 P1 — Important (explicitly promised, not yet built)

**3. HPA/DCO (Host Protected Area / Device Configuration Overlay) detection & removal** — zero
implementation found. Data can hide here and survive a "complete" wipe.

**4. Carving coverage is narrower than promised.** Playbook asked for 5–10 types; only 6 are done
(JPEG/PNG/GIF/BMP/PDF/ZIP). MP4 and DOCX (structure-aware, not just generic ZIP) are missing.

**5. No classification of recovered files** (ML-based or even simple rule-based tagging by
content category) — explicitly deferred in `DECISIONS.md`, still outstanding.

**6. Report export missing XML.** Only PDF + JSON exist; playbook and `PROJECT.md` both call for
JSON *and* XML for case-management system integration.

**7. IEEE 2883-2022 and ACPO/ISO 27037 are not represented as real standards in code** — they're
mentioned in documentation only, with no corresponding `WipeStandard`/spec entry or explicit
mapping surfaced in the Compliance Matrix. Only NIST 800-88 and DoD 5220.22-M actually exist as
enforced code paths.

### 🟡 P2 — Roadmap phases not yet started (README confirms this honestly)

Per the project's own status table, **Phases 12–15 have not started**:

- **Phase 12** — Companion read-only web dashboard
- **Phase 13** — Security hardening & adversarial testing (privilege boundaries, write-blocking
  audit, input validation on device/path selectors) — no written hardening test log exists yet
- **Phase 14** — Full QA pass against public forensic validation datasets (e.g., NIST CFReDS test
  images) — all current testing uses synthetic image files, not recognized forensic corpora
- **Phase 15** — Packaging (native installers), user manual, technical/architecture docs,
  performance evaluation report — none of these deliverables exist yet

### 🟢 P3 — Lower priority / intentionally deferred

- **Gutmann 35-pass mode** — not implemented, but this is first in the project's own documented
  cut-order (safe to skip; NIST/DoD coverage is considered sufficient)
- **macOS support** — out of scope per `PROJECT.md` §7 cut order (Windows + Linux only for now)
- **Asymmetric signing (Ed25519)** — currently HMAC-SHA256 with a per-install symmetric key, which
  is fine for single-install chain-of-custody but weaker for multi-party/court-facing
  non-repudiation; explicitly deferred pending a cloud key-hierarchy decision
- **Live/real hardware validation** — all wipe/recovery testing so far runs against image files
  only, which is correct for safe development but still needs to happen once before a genuine
  production-readiness claim

---

## 4. Priority Roadmap (Backend-First)

1. **Implement ATA Secure Erase / NVMe Sanitize** (or, at minimum, the UI acknowledgment gate the
   rules already require when falling back to overwrite)
2. **Build the file-eraser metadata/trace scrubbing sub-module** (registry, thumbnails, journal,
   slack space) — the module currently under-delivers on its own stated scope
3. **HPA/DCO detection and removal**
4. **Expand the signature database** (MP4, structure-aware DOCX) and add basic rule-based
   classification of recovered files
5. **Add XML report export** alongside existing PDF/JSON
6. **Phase 13 — security hardening & adversarial testing pass**, with a written findings log
7. **Phase 14 — validate against NIST CFReDS (or equivalent) public forensic datasets**
8. **Phase 15 — packaging, user manual, technical docs, performance report** (release readiness)
9. Optional/lower priority: companion web dashboard, macOS support, asymmetric signing

---

## 5. Bottom Line

- **Audit, reporting, GUI, persistence, and cross-platform layers**: mature, well-tested, and in
  several places (confidence scoring, bifragment gap carving, tamper-evident signing) genuinely
  ahead of a typical hackathon-stage build.
- **The "hard" forensic/hardware layer** — SSD-safe erasure, HPA/DCO, metadata-level scrubbing —
  is the part that's still missing, and it happens to be exactly the part the playbook and the
  project's own rules treat as non-negotiable. This is where engineering effort should go next,
  not the dashboard or the reporting pipeline.
- README status claims ("Phase 3 Complete", etc.) should be read carefully against this report —
  some phase-completion claims cover the overwrite-based subset of a feature, not the full
  hardware-backed scope the original checkpoint asked for.
