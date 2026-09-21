# Purgent — Remaining Gaps: How to Solve Them (per `RULES.md`)

This file is a **do-list**, not a status report. Each item below maps to an open entry in
`KNOWN_ISSUES.md` / `SECURITY_REVIEW.md`, with the exact rule from `RULES.md` that governs how it
must be closed. **An item is not done when the code is written — it's done when the evidence rule
next to it is satisfied.** That's not a suggestion, it's `RULES.md` §7/§8.

---

## 1. CFReDS Forensic Dataset Validation (KO-2)

**Governing rule:** §7 — *"Recovery/carving accuracy claims must cite what they were measured
against... 'tested' means tested against synthetic files until validated against a recognized
forensic dataset (e.g., NIST CFReDS); the two must not be conflated."*

**Steps:**
1. Download a public corpus image from NIST CFReDS (cfreds.nist.gov) or an equivalent dataset with
   cataloged deleted files. Start with a small corpus, not a full disk image.
2. Convert/extract to a raw image (`.img`/`.dd`) — the carving engine reads raw bytes only.
3. If the corpus ships a file catalog, build a manifest: one expected SHA-256 per line.
4. Run the existing (currently `#[ignore]`d) harness:
   ```sh
   set PURGENT_CFREDS_IMAGE=C:\forensic\corpus.img
   set PURGENT_CFREDS_MANIFEST=C:\forensic\manifest.sha256   # optional
   cargo test -p purgent-core --release -- --ignored carve_against_cfreds_image --nocapture
   ```
5. Copy the printed summary (scanned bytes, recovered count, precision/recall) into a new dated
   entry in `FORENSIC_VALIDATION.md`, using the template already in that file.
6. Note per-signature coverage (jpeg/png/gif/bmp/pdf/mp4/docx/zip) and whether a fragmented case
   was exercised.

**Only then:** close KO-2 in `KNOWN_ISSUES.md` and reference the `FORENSIC_VALIDATION.md` entry
as evidence. Do not just mark it "done" — link the entry.

**Risk level:** Low. No hardware risk — this only touches an image file.

---

## 2. Real Hardware Validation (KO-1)

**Governing rule:** §4 — *"Test destructively only against virtual disks, loopback devices, or
disk image files, never against a real production drive... and even then, only against
disposable/scratch media that holds no data of value."* Also §7 — a green `cargo test` is **not**
evidence for ATA/NVMe/HPA-DCO correctness; only a recorded live-hardware run is.

**Steps:**
1. Get a **disposable** USB drive or spare SSD/HDD — nothing with data you care about. This will
   be permanently destroyed.
2. Record the **before** state: `hdparm -I` (Linux) or an `IDENTIFY DEVICE` dump — model, firmware,
   capacity, security feature support word, current HPA/DCO state.
3. Run Purgent against that device and try each method once it's applicable:
   - ATA Secure Erase (SATA device)
   - NVMe Sanitize (NVMe device, if you have one to spare)
   - HPA/DCO removal (artificially set an HPA first via `hdparm -N` to have something to remove)
4. Record the exact command issued (copy the log line from Purgent's own operation log).
5. Record the **after** state (re-query `IDENTIFY DEVICE`) and how you verified the result
   (read-back sample / hex dump / controller status).
6. Repeat on both Windows and Linux if you have access to both — each OS is a separate entry.
7. Paste each run into `HARDWARE_VALIDATION.md` using the template already there — **including
   failed or unexpected runs**. A recorded failure is required by the rules just as much as a
   recorded pass; don't only log successes.

**Only then:** close KO-1, and only for the specific method/OS combination actually run. Don't
generalize "ATA Secure Erase verified" to NVMe or to the other OS from a single run — §7 requires
evidence per method, not per feature area.

**Risk level:** High (irreversible on the test device). Do this deliberately, not casually — wrong
device selection here is exactly the failure mode the confirmation flow exists to prevent, so
triple-check the target before every run.

---

## 3. Security Hardening & Fuzzing (KO-3)

**Governing rule:** §4 — *"Security-relevant code (wiping, hashing, signing, write-blocking)
requires the hardening pass in Phase 13... before it can be considered release-ready."*

**Steps (mirrors the checklist already in `SECURITY_REVIEW.md`):**
1. Set up `cargo-fuzz` (nightly toolchain) and write fuzz targets for:
   - `ata_task_file_builder` — feed it arbitrary byte slices, confirm no panic/UB
   - `nvme_admin_cmd_builder` — same, for the 40-byte CDW region
   - `parse_identify` — feed malformed/truncated 512-byte IDENTIFY payloads
2. Run each target for a meaningful duration (minutes-to-hours / ≥1M iterations) with zero
   crashes. Record the run duration and iteration count.
3. Manually audit every device/path IPC entry point (`list_devices`, `erase_path`, `erase-file`,
   `recover` source/output selectors) for path traversal or injection. Confirm the existing guards
   (`verify_or_refuse_path`, read-only carve source, fresh recovery output dir) actually hold under
   adversarial input, not just happy-path input.
4. Confirm there's no "enumeration vs execution" drift: when a device is confirmed by the operator
   and then erased, re-open it by ID and compare serial/model right before the destructive call,
   not just at selection time.
5. Write every finding — pass or fail — into `SECURITY_REVIEW.md`'s Findings section. Fix anything
   that fails before it's considered closed; if something is accepted as a known limitation instead
   of fixed, say so explicitly with reasoning (this itself must go through §5's known-issues
   tracking).

**Only then:** close KO-3, referencing the findings section, not just "checklist done."

**Risk level:** Low-medium — fuzzing runs locally against isolated functions; no real device
needed. Do this before hardware validation if a fuzz-found bug would otherwise be caught the hard
way on real hardware.

---

## 4. Ed25519 Asymmetric Signing (KO-4)

**Governing rule:** §3 — *"The signing model is currently symmetric (HMAC-SHA256, per-install
key)... Documentation and in-app report exports must disclose this limitation explicitly until an
asymmetric scheme (e.g., Ed25519) is added."*

**Steps:**
1. This one is **not blocking** — the current disclosure already satisfies §3 as long as it stays
   accurate. Treat this as a feature to build, not a compliance failure to rush.
2. Design: keep HMAC-SHA256 as a fast local-integrity check, add Ed25519 as a second, exportable
   signature over the same canonical report payload — so a third party can verify authenticity
   using a published public key, without needing the machine's private HMAC key.
3. Key management: generate the Ed25519 keypair on first run (same place the HMAC key is
   generated), never log/store the private key in plaintext outside the local key store (per §2).
4. Update `reporting.rs` to attach both signatures; update the report schema/version so old reports
   (HMAC-only) remain valid and distinguishable from new ones.
5. Add tests proving: signature verifies against the correct public key, fails against a tampered
   payload, fails against the wrong key.
6. Update `FAQ`/README copy once implemented — don't leave the "on the roadmap" language after it
   ships (§5 requires docs to track reality, in both directions).

**Only then:** close KO-4.

**Risk level:** Low — pure software change, standard `cargo test` discipline applies (§8).

---

## 5. Enable a Real Content-Security-Policy (new finding, not yet in `KNOWN_ISSUES.md`)

**Governing rule:** Falls under §4's general "security-relevant code" discipline and §5's
transparency rule — an intentionally disabled security control should be tracked, not silent.

**Steps:**
1. `src-tauri/tauri.conf.json` currently has `"security": { "csp": null }` — CSP is fully off.
2. Define a real policy: restrict `default-src`/`script-src` to `'self'` and only the assets the
   app actually loads (no remote script sources should be needed for a fully offline Tauri app).
3. Test the built app after enabling CSP — a wrongly scoped policy can silently break the webview
   (e.g., inline styles/scripts). Verify the UI still renders and functions fully.
4. Add this item to `KNOWN_ISSUES.md` **now** (as open) so it's tracked per §5, then close it once
   the policy is in place and verified.

**Risk level:** Low, but easy to break the UI if scoped incorrectly — test thoroughly before
merging.

---

## 6. Linux Trace-Scrubber Verification (documented limitation)

**Governing rule:** §7 — evidence must match the platform claimed; §5 — known limitations must be
documented, not discovered in production.

**Steps:**
1. On an actual Linux machine (not cross-compiled/assumed), run the file eraser against real
   `.bash_history` and Trash paths.
2. Confirm the "atomic rewrite" and "exact-match" behavior described in `trace_scrubber.rs` holds
   on a real filesystem (ext4 at minimum).
3. Record the result — pass or fail — in `KNOWN_ISSUES.md`, replacing the current "cannot be
   verified on this host" note with an actual verified/failed status.

**Risk level:** Low — test on a disposable Linux VM/user account, not your main system's real
shell history.

---

## 7. Phase 15 — Packaging & Docs (lowest priority, do last)

**Governing rule:** §8 — nothing here is "done" until the full verification loop (build, test,
fmt, clippy, frontend build) passes for the packaged artifact too, not just the dev build.

**Steps:**
1. Native installers: MSI/NSIS (Windows), `.deb`/AppImage (Linux) via Tauri's bundler config.
2. User manual: plain walkthrough of each tab (Wipe/Erase/Recover/Reports) for a non-technical
   operator.
3. Architecture/technical docs: consolidate `PROJECT.md`/`DECISIONS.md`/`AGENTS.md` into a single
   reader-facing technical doc.
4. Performance benchmark report: wipe throughput (MB/s per standard), recovery scan speed, false
   positive rate on the CFReDS run from item 1.

**Risk level:** None — packaging and docs, no destructive operations involved.

---

## Order of Operations (recommended)

1. **CFReDS validation** (#1) — safe, closes a real gap, no hardware risk
2. **Security hardening / fuzzing** (#3) — safe, local-only, catches bugs before they hit hardware
3. **CSP fix** (#5) — safe, quick, but test the UI after
4. **Hardware validation** (#2) — do this carefully, only once 1–3 give you confidence the code
   underneath is solid
5. **Linux trace-scrubber check** (#6) — whenever Linux access is available
6. **Ed25519 signing** (#4) — feature work, no urgency, do when there's time
7. **Phase 15 packaging/docs** (#7) — last, once everything above is evidence-backed

---

## Reminder — What "Done" Actually Means Here (RULES.md §8)

For every item above, closing it means:
- `cargo check`/`cargo build` passes for the whole workspace, not just the touched crate
- `cargo test --workspace` passes in full
- `cargo fmt --check` and `clippy` are clean
- `npm run build` passes if any frontend file was touched
- The specific evidence artifact for that item exists (a `FORENSIC_VALIDATION.md` entry, a
  `HARDWARE_VALIDATION.md` entry, a `SECURITY_REVIEW.md` findings entry) — not just a checked box
- Any new issue discovered along the way gets logged in `KNOWN_ISSUES.md`, not silently fixed or
  silently ignored

A todo list item checked off without its evidence artifact is, per the project's own rules, not
actually finished.
