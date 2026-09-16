# RULES.md — Purgent Project Rules

This file governs **how Purgent may and may not be built, tested, and used**, independent of
which build phase is active. `AGENTS.md` tells a coding agent *what to build next and in what
order*; this file tells everyone (agents, contributors, and users) the boundaries that never move
regardless of phase or deadline pressure. Where any other document conflicts with this file, this
file wins.

---

## 1. What This Project Is Allowed To Do

- Securely erase data the operator has explicit authorization to erase (their own devices, or
  devices/media they are authorized to sanitize on behalf of an organization/client).
- Recover data from media the operator has explicit authorization to analyze (their own devices,
  or media provided to them under a forensic engagement, investigation, or organizational data
  recovery request).
- Generate signed, exportable reports documenting what was done, to whom/what, when, and by whom.
- Operate fully offline, with cloud sync (Supabase, via `sync.rs`) as a strictly optional,
  opt-in layer, disabled by default until the operator sets `sync_optin.txt` and the
  `PURGENT_SUPABASE_URL` / `PURGENT_SUPABASE_ANON_KEY` environment variables.

## 2. What This Project Must Never Do

- **Must never be built or shipped with a bypass for the authorization/confirmation step.** Every
  wipe operation requires the operator to actively confirm the specific target (exact
  model/serial/path match) in-session. There is no configuration flag, CLI switch, environment
  variable, or "trusted mode" that skips this — on any platform, in any build.
- **Must never target or scan a device without the operator initiating that specific action.** No
  background scanning, no auto-discovery-and-wipe, no "clean up now" default actions, and no
  scheduled/unattended wipes of any kind.
- **Must never transmit recovered file contents anywhere off the local machine automatically.**
  The optional Supabase sync path (`sync.rs` → `AuditSyncClient`) may only upload one audit-metadata
  row per report (operation ids, type, operator, target, standard, capacity/timestamps,
  verification status, report hash, signature) to `{url}/rest/v1/audit_logs`. It must never gain a
  code path that uploads carved/recovered file bytes, full disk images, or raw report payloads
  beyond that metadata row — this applies even if a future feature requests "richer" cloud sync.
- **Must never present a wipe as complete without a passed verification (read-back) step**, and
  must never present a recovery result without its per-factor confidence score attached.
- **Must never silently fall back from a hardware-backed method (ATA Secure Erase / NVMe Sanitize)
  to blind overwrite on flash media.** If the hardware method is unavailable or refused by the
  controller, the operation must return `WipeError::RequiresFallbackAck` and halt; it may only
  proceed to overwrite after the operator explicitly acknowledges, and the report must record
  `method: overwrite` plus the fallback reason. This is implemented today — no future change may
  remove or bypass the acknowledgment gate.
- **Must never present an ATA/NVMe hardware-erase or HPA/DCO-removal code path as "verified" or
  "production-ready" in documentation, release notes, or in-app copy unless it has been exercised
  end-to-end against at least one real physical device on that OS**, with the result recorded per
  §7 below. Passing `cargo test` / compiling cleanly is not sufficient evidence for this claim.
- **Must never be marketed or documented with unverified absolute claims** ("100% recovery,"
  "guaranteed unrecoverable," "court-admissible" without naming the standard/method that backs the
  claim). Every claim in user-facing copy must trace to a named standard, a measured result, or an
  explicitly disclosed limitation (see §7 on the current HMAC-only signing model).
- **Must never log, store, or transmit credentials, signing keys, API tokens, or the operator's
  HMAC signing key in plaintext,** in source control, in logs, or in generated reports.
- **Must never be used as the basis for a tool that automates mass, unauthorized data destruction
  or mass, unauthorized data extraction.** Any feature request that points in that direction is
  out of scope for this repository, full stop — not "postponed," rejected.

## 3. Legal & Ethical Use Boundaries

- Purgent is built for **authorized** use: sanitizing devices you own or are contracted to
  sanitize, and recovering/analyzing media you own or are engaged to examine. It is not built for,
  and must not be adapted for, accessing or wiping devices without the owner's or a lawful
  authority's permission.
- Every operation must log an **operator identity** for accountability. A build that removes or
  makes optional the operator-ID field on any destructive or recovery operation is non-compliant
  with this project's rules, regardless of what phase requested it.
- Chain-of-custody integrity for recovery results (read-only source mounting, immediate SHA-256
  hashing at extraction, tamper-evident HMAC-signed reports) is a legal-admissibility concern, not
  a nice-to-have — it must never be weakened to save development time.
- The signing model is currently **symmetric (HMAC-SHA256, per-install key)**. This is adequate
  for single-install chain-of-custody but does **not** provide multi-party non-repudiation — anyone
  holding the signing key can also forge a report. Documentation and in-app report exports must
  disclose this limitation explicitly until an asymmetric scheme (e.g., Ed25519) is added; the
  product must never imply a third party can verify authenticity without trusting the originating
  machine.
- Include a clear usage/liability disclaimer in the shipped product and README: the tool does not
  itself authorize any action; the operator is responsible for having the legal right to perform
  it on the target device/media.

## 4. Development Conduct Rules

- **Test destructively only against virtual disks, loopback devices, or disk image files**, never
  against a real production drive, unless a human has given a specific, separate, explicit
  instruction to test against real hardware — and even then, only against disposable/scratch
  media that holds no data of value (see §7 for how such a validation run must be recorded).
- **No feature ships without its corresponding verification/reporting behavior.** A wipe module
  without verification, or a recovery module without confidence scoring and hashing, is not a
  partial feature — it does not ship.
- **No standard is claimed as "supported" until it has been validated against its actual
  specification** (NIST SP 800-88 Rev.1, DoD 5220.22-M, IEEE 2883-2022, ISO/IEC 27037), not just
  implemented from memory of what the standard "probably" requires.
- **Cross-platform low-level code must be isolated behind the Storage/Device Abstraction Layer**
  (`crates/purgent-core/src/modules/storage/`). No OS-specific disk-access call (`AtaPassThroughEx`,
  `HDIO_DRIVE_TASKFILE`, raw ioctls, etc.) may appear outside `storage/windows.rs`,
  `storage/linux.rs`, `storage/secure_erase.rs`, or `storage/hpa_dco.rs`.
- **macOS is explicitly out of scope for now** (per `PROJECT.md` §7). No code may silently
  compile a broken or unimplemented macOS path; any `cfg(target_os = "macos")` branch introduced
  before a real macOS port is scoped must fail loudly (compile error or explicit "unsupported"
  runtime error), never silently no-op or fall through to Linux/Windows behavior.
- **Dependencies are reviewed before adoption.** Do not add a forensic/parsing library whose
  license is incompatible with this project's licensing, and do not vendor GPL/AGPL forensic tool
  source directly (see `AGENTS.md` §2) — reference implementations only.
- **Production (non-test) code paths must not use `.unwrap()`, `.expect()`, or `panic!()` on any
  fallible operation.** Errors must propagate as typed `Result`/error-enum variants that reach the
  operator and, where relevant, get recorded in the report. `.unwrap()`/`.expect()` are permitted
  only inside `#[cfg(test)]` modules or on operations that are structurally infallible (e.g.
  `size_of::<T>() as u64`), and any such use must be commented explaining why it cannot fail.
- **The Rust toolchain version must be pinned and documented**, not left to "whatever `cargo`
  happens to be installed." Add/maintain a `rust-toolchain.toml` and state the minimum supported
  Rust version (MSRV) in `README.md`, so the `edition2024`-requiring dependency chain (`ureq` →
  `rustls` → `zeroize`/`idna_adapter`) doesn't surprise contributors on older distro-packaged Rust.
- **Security-relevant code (wiping, hashing, signing, write-blocking) requires the hardening pass
  in Phase 13 of `AGENTS.md` before it can be considered release-ready**, even if it "works" in an
  earlier phase's demo.

## 5. Documentation & Transparency Rules

- Every recognized standard the tool claims to support must appear in the Compliance Matrix
  (`PROJECT.md` §6), and the matrix must be updated in the same change that adds or modifies
  support for a standard — not retroactively.
- User-facing documentation must clearly state the difference in reliability between hardware-
  backed erase (ATA Secure Erase/NVMe Sanitize) and overwrite-based erase, and must clearly state
  that recovery confidence scores are estimates, not guarantees.
- Any known limitation (e.g., SSD wear-leveling effects, recovery accuracy on heavily fragmented
  files, the current HMAC-only signing model, lack of macOS support) must be documented in the
  user manual, not only discovered by the user in production.
- **Known correctness issues in shipped code must be tracked, not buried.** Any defect discovered
  in a security-relevant path (e.g., an incorrect ATA flag constant) must be logged in
  `DECISIONS.md` *and* surfaced as an open item in a `KNOWN_ISSUES.md`-style list until fixed and
  re-validated — it may not be fixed silently in a later commit with no trace of the interim risk.

## 6. Change Control

- Changes to this file itself require the same decision-logging discipline as any other
  non-trivial decision: record the change and its reasoning in `DECISIONS.md`.
- No rule in this file may be relaxed by an agent unilaterally to unblock a build phase. If a rule
  in this file appears to conflict with a request, the agent must surface the conflict to the user
  rather than silently choosing one side.
- A change that reduces the passing test count, removes a fallback-acknowledgment gate, or weakens
  the exact-target confirmation flow must be treated as a rules violation, not an ordinary
  refactor, and must be called out explicitly in the PR/commit description.

## 7. Verification & Evidence Discipline

*(New section — codifies what "done" actually means for this project, based on the gap between
what compiles/tests-green and what has been verified against reality.)*

- **A green `cargo test` run is evidence of internal logical consistency, not of real-world
  correctness**, for anything touching physical hardware (ATA/NVMe pass-through, HPA/DCO). These
  paths require a documented live-hardware run (device model, OS, exact command issued, before/after
  `IDENTIFY DEVICE` state, verification method) before being described as "done" anywhere outside
  an internal TODO.
- **Recovery/carving accuracy claims must cite what they were measured against.** "Tested" means
  tested against synthetic files until validated against a recognized forensic dataset (e.g., NIST
  CFReDS); the two must not be conflated in status reports or README claims.
- **Every unresolved P0/P1 item from an internal gap-analysis pass must have a corresponding entry
  in this file's rules, `DECISIONS.md`, or a tracked issue** — a gap that is found and then not
  tracked anywhere is treated the same as a rules violation under §5's "known issues" requirement.

## 8. Post-Task Verification Loop (mandatory for any agent — human or AI)

*(New section — closes the gap between "I finished the todo list" and "the project actually
works." No task, phase, or todo list is complete until this loop has been run.)*

- **Finishing a todo list is not the same as finishing the work.** After every set of changes
  (a phase, a todo list, a bugfix batch, a PR), the agent must stop and independently verify the
  *whole* project still works — not just the file(s) that were touched. "I made the edit" is not a
  completion signal; "I confirmed the project builds, tests pass, and the specific behavior I
  changed does what it's supposed to" is.
- **Minimum verification pass, every time, before declaring anything done:**
  1. `cargo check` / `cargo build` across the whole workspace (not just the crate touched).
  2. `cargo test --workspace` — full suite, not a single module's tests.
  3. `cargo fmt --check` and any configured lints (`clippy`, etc.).
  4. Frontend: `npm run build` (tsc + vite) if any frontend file was touched, or if a Rust change
     affects an IPC command signature the frontend depends on.
  5. For any change touching a hardware path (`storage/secure_erase.rs`, `storage/hpa_dco.rs`,
     `storage/windows.rs`, `storage/linux.rs`): re-confirm against §7's live-hardware evidence
     requirement — a fix here is not "done" on green tests alone.
- **If verification fails, that is not a stopping point — it is the actual task.** The agent must
  treat the failure as the next item to fix, immediately re-run the same verification loop, and
  repeat until everything genuinely passes. Do not report partial success, move on to unrelated
  work, or hand a broken state back to the user with "should be mostly working" language. Keep
  looping — diagnose, fix, re-verify — until the whole project is confirmed working end to end.
- **Act like a senior security engineer reviewing someone else's PR, not like the person who wrote
  it.** Be skeptical of your own change by default: assume it introduced a regression until proven
  otherwise by an actual passing run, not by re-reading the diff and feeling confident about it.
  Specifically:
  - Don't trust a compile pass as proof of correctness for security-relevant code (wiping,
    signing, hashing, ATA/NVMe pass-through, HPA/DCO, write-blocking).
  - Actively look for what the change could have broken elsewhere (a shared struct field, an IPC
    contract the frontend relies on, a report schema an exporter depends on) — not just whether
    the immediate function under edit looks right.
  - Prefer disproving your own fix over confirming it. Try to make it fail before declaring it
    fixed.
  - If a fix reveals a second, adjacent issue, do not silently scope-creep past it or silently
    ignore it — log it (per §5's known-issues rule) and fix it in the same loop if it blocks
    correctness, or explicitly flag it as a separate tracked item if it doesn't.
- **A task is only "done" when:** the full verification pass in this section is green, any
  hardware-touching change has real-device evidence per §7, and any newly discovered issue is
  either fixed or explicitly logged — not when the todo list has been checked off.