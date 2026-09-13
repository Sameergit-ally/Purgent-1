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
- Operate fully offline, with cloud sync as a strictly optional, opt-in layer.

## 2. What This Project Must Never Do

- **Must never be built or shipped with a bypass for the authorization/confirmation step.** Every
  wipe operation requires the operator to actively confirm the specific target in-session. There
  is no configuration flag, CLI switch, or "trusted mode" that skips this.
- **Must never target or scan a device without the operator initiating that specific action.** No
  background scanning, no auto-discovery-and-wipe, no "clean up now" default actions.
  the operator to actively confirm the specific target in-session. No configuration flag, CLI
  switch, or "trusted mode" may skip this.
- **Must never transmit recovered file contents anywhere off the local machine automatically.**
  Optional cloud sync carries audit metadata only (operation type, device identifiers, hashes,
  timestamps, scores) — never the recovered file payload — unless the operator takes a separate,
  explicit export/upload action per file.
- **Must never present a wipe as complete without a passed verification step**, and must never
  present a recovery result without its confidence score.
- **Must never silently fall back from a hardware-backed method (ATA Secure Erase / NVMe Sanitize)
  to blind overwrite on flash media.** If the hardware method is unavailable, the tool must say so
  and require explicit operator acknowledgment before proceeding with a less reliable method.
- **Must never be marketed or documented with unverified absolute claims** ("100% recovery,"
  "guaranteed unrecoverable" without naming the standard/method that backs the claim). Every claim
  in user-facing copy must trace to a named standard or a measured result.
- **Must never log, store, or transmit credentials, signing keys, or API tokens in plaintext,**
  in source control, in logs, or in generated reports.
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
- Chain-of-custody integrity for recovery results (read-only source mounting, immediate hashing,
  tamper-evident reports) is a legal-admissibility concern, not a nice-to-have — it must never be
  weakened to save development time.
- Include a clear usage/liability disclaimer in the shipped product and README: the tool does not
  itself authorize any action; the operator is responsible for having the legal right to perform
  it on the target device/media.

## 4. Development Conduct Rules

- **Test destructively only against virtual disks, loopback devices, or disk image files**, never
  against a real production drive, unless a human has given a specific, separate, explicit
  instruction to test against real hardware.
- **No feature ships without its corresponding verification/reporting behavior.** A wipe module
  without verification, or a recovery module without confidence scoring and hashing, is not a
  partial feature — it does not ship.
- **No standard is claimed as "supported" until it has been validated against its actual
  specification** (NIST SP 800-88 Rev.1, DoD 5220.22-M, IEEE 2883-2022), not just implemented from
  memory of what the standard "probably" requires.
- **Cross-platform low-level code must be isolated behind the Storage/Device Abstraction Layer.**
  No OS-specific disk-access call may appear outside that layer.
- **Dependencies are reviewed before adoption.** Do not add a forensic/parsing library whose
  license is incompatible with this project's licensing, and do not vendor GPL/AGPL forensic tool
  source directly (see `AGENTS.md` §2) — reference implementations only.
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
  files) must be documented in the user manual, not only discovered by the user in production.

## 6. Change Control

- Changes to this file itself require the same decision-logging discipline as any other
  non-trivial decision: record the change and its reasoning in `DECISIONS.md`.
- No rule in this file may be relaxed by an agent unilaterally to unblock a build phase. If a rule
  in this file appears to conflict with a request, the agent must surface the conflict to the user
  rather than silently choosing one side.
