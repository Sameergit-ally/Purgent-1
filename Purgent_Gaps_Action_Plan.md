# Purgent — Gap Closure Action Plan

**Generated:** September 16, 2026
**Source:** Independent verification (frontend build tested, Rust core statically reviewed, code cross-checked against `DECISIONS.md` / `Purgent_Project_Verification_Report.md`)
**Scope:** Everything confirmed to still be broken, missing, or unverified as of this date.

---

## How to use this file

- Work top to bottom — **P0 items block a real production/legal-use claim.**
- Check off `[ ]` → `[x]` as each item is actually fixed and *re-verified* (not just edited).
- Don't mark "Done" on a hardware item without a real test run on physical media — a compiling change is not a fix here.

---

## 🔴 P0 — Blocking issues (fix before any real-device use)

### 1. Windows ATA Secure Erase — wrong `AtaFlags` value
- **File:** `crates/purgent-core/src/modules/storage/secure_erase.rs:271`
- **Problem:** Data-out ATA pass-through command sets `AtaFlags: 0x40`. Per the ATA pass-through IOCTL spec, `0x40` is unrelated to data direction — the correct data-out flag is `0x04` (`ATA_FLAGS_DATA_OUT`). Currently the flag byte is effectively wrong for any data-carrying ATA command in this path.
- **Risk:** A secure-erase command issued with the wrong data-direction flag can silently fail, hang, or behave unpredictably on real hardware — undermining the entire "verified erase" claim in the signed report.
- **Fix steps:**
  - [x] Change `AtaFlags` for the data-out branch from `0x40` to `0x04`.
  - [x] Re-check the data-in branch and the no-data branch for the same class of mistake (audit **all** `AtaFlags` usages in this file, not just line 271).
  - [x] Cross-reference against Microsoft's `ATA_PASS_THROUGH_EX` documentation before changing.
  - [x] Add a unit test that asserts the exact byte value of `AtaFlags` per command type (catches silent regressions).
- **Acceptance criteria:** Test asserts `0x04` on data-out path; a real (or emulated) ATA pass-through call succeeds against at least one physical or virtualized SATA device.
- **Status (16 Sep 2026):** CODE FIXED — constants centralised in `hpa_dco.rs` (`ATA_FLAGS_DATA_OUT = 0x04`), pure `ata_flags()` helper in `secure_erase.rs`, two byte-assert tests added, `cargo test --workspace` = 82 passed. Windows host has no ATA-capable container to run a real pass-through call → the second half of the acceptance criteria stays **pending** (see P0 #2).

### 2. Zero live-hardware validation
- **Files:** `storage/secure_erase.rs`, `storage/hpa_dco.rs`, `storage/windows.rs`, `storage/linux.rs`
- **Problem:** ATA Secure Erase, NVMe Sanitize, and HPA/DCO removal ioctl paths are **compile-validated only** — never executed against a real drive on real Windows/Linux. The `DECISIONS.md` log admits this directly ("Live-ioctl paths are unverifiable on the Windows dev host").
- **Risk:** Any of these code paths could be functionally broken (wrong struct packing, wrong register order, wrong HOB bit handling) and nobody would know until a real drive is wiped.
- **Fix steps:**
  - [ ] Set up a disposable test drive (cheap USB SSD/HDD or a spare device — never anything with real data) on both Windows and Linux.
  - [ ] Run ATA Secure Erase end-to-end; verify with read-back + a hex dump that the drive is actually wiped.
  - [ ] Run NVMe Sanitize end-to-end on an NVMe device.
  - [ ] Run HPA/DCO detection + removal on a drive with an artificially set HPA (can be created with `hdparm` on Linux) and confirm the removed capacity is correctly reported by `IDENTIFY DEVICE` afterward.
  - [ ] Document each run (device model, OS, result, screenshots/logs) in `HARDWARE_VALIDATION.md`.
- **Acceptance criteria:** At least one successful, documented real-device run per method (ATA Secure Erase, NVMe Sanitize, HPA removal, DCO removal) on both Windows and Linux.
- **Status (16 Sep 2026):** Run template + procedure added in `HARDWARE_VALIDATION.md`; no physical media available in this environment. Stays **open** until a real run is logged.

---

## 🟠 P1 — Should fix before calling this "forensic-grade"

### 3. No macOS support
- **Problem:** `target_os = "macos"` does not appear anywhere in `purgent-core`. README/`PROJECT.md` mark this as an intentional cut, but it limits the "cross-platform" claim.
- **Fix steps:**
  - [x] Decide: stay Windows+Linux only (update all marketing copy to stop implying full cross-platform), OR scope a macOS port (IOKit-based drive enumeration + secure erase equivalent).
  - [x] If deferring, add an explicit `#[cfg(target_os = "macos")] compile_error!(...)` so it fails loudly instead of silently, if it's ever attempted.
- **Acceptance criteria:** Either macOS builds and passes the same test suite, or the "not supported" state is impossible to build silently-wrong on.
- **Status (16 Sep 2026):** **Decided — stay Windows + Linux.** Hardware paths already return `HardwareEraseError::Unsupported` at runtime on non-Windows/Linux (explicit loud failure, no silent no-op or fallthrough), so the `compile_error!` gate was judged unnecessary and *not* added — a mac build of unrelated modules stays possible while destructive paths still refuse loudly. Decision logged in `DECISIONS.md` [2026-09-16]. Marketing copy never claimed macOS (README states Windows/Linux).

### 4. No forensic-dataset validation for file recovery/carving
- **Problem:** All carving/recovery tests run against synthetic, hand-built test files — never a recognized forensic corpus (e.g., NIST CFReDS).
- **Fix steps:**
  - [ ] Download a CFReDS (or equivalent) disk image with known, cataloged deleted files.
  - [ ] Run the recovery engine against it; compare recovered files + confidence scores against the known ground truth.
  - [ ] Record precision/recall (false positives, false negatives, fragmentation-reconstruction accuracy) in `FORENSIC_VALIDATION.md`.
- **Acceptance criteria:** A written report showing recovery accuracy against at least one public forensic dataset.
- **Status (16 Sep 2026):** env-gated, `#[ignore]`-by-default test harness added (`carve_against_cfreds_image` in `recovery.rs`); a corpus download + run is required — remains **open** until a measured result is recorded.

### 5. No security hardening / adversarial testing
- **Problem:** No fuzzing of ATA/NVMe pass-through payload builders, no documented privilege-boundary review, no input-validation audit on device/path selectors.
- **Fix steps:**
  - [ ] Fuzz the ATA/NVMe command-builder functions (e.g., with `cargo-fuzz`) for panics/UB on malformed input.
  - [ ] Review every place a device path or file path comes from the UI/IPC layer for path traversal or injection risk.
  - [ ] Confirm the app cannot be tricked into targeting a device other than the one the operator confirmed (race conditions between enumeration and execution).
  - [ ] Write up findings in `SECURITY_REVIEW.md`.
- **Acceptance criteria:** Fuzz targets run for a meaningful duration with zero crashes; hardening findings documented and triaged.
- **Status (16 Sep 2026):** audit checklist + planned fuzz targets documented in `SECURITY_REVIEW.md`; no fuzz run yet (needs nightly `cargo-fuzz`). Remains **open**; IPC path-selector audit review in progress/next pass.

### 6. Signing is symmetric only (HMAC-SHA256)
- **Problem:** Reports are signed with a per-install HMAC key. Fine for single-install chain-of-custody, weak for court-facing / multi-party non-repudiation (anyone with the key can also forge a report).
- **Fix steps:**
  - [ ] Decide on an Ed25519 (or similar asymmetric) signing scheme.
  - [ ] Design key provisioning/distribution (where does the private key live, how is the public key distributed for verification).
  - [ ] Implement alongside HMAC (don't break existing `verify_report` compatibility) or provide a migration path.
- **Acceptance criteria:** Reports can be verified with only a public key, without trusting the signing machine.

---

## 🟡 P2 — Roadmap / polish (not launch-blocking, but currently just missing)

- [ ] **Companion read-only web dashboard** — not started at all.
- [ ] **Packaging** — no native installers (MSI/NSIS for Windows, `.deb`/AppImage for Linux) exist yet.
- [ ] **User manual** — end-user-facing docs (as opposed to `USAGE_GUIDE.md`, which is more technical) don't exist.
- [ ] **Architecture/technical docs** — beyond the README diagram, no deeper architecture doc for contributors.
- [ ] **Performance evaluation report** — no benchmarks (wipe throughput, carving speed on large images, etc.) documented anywhere.

---

## 🟢 P3 — Explicitly deferred (low priority, acknowledge and move on)

- [ ] Gutmann 35-pass overwrite mode — NIST/DoD coverage is already considered sufficient; only implement if a specific compliance requirement demands it.

---

## Minor / housekeeping (found during this review, not previously flagged)

- [ ] **No pinned Minimum Supported Rust Version (MSRV).** CI uses `dtolnay/rust-toolchain@stable` (always-latest), which works but means local dev environments need a fairly recent Rust (the dependency tree — via `ureq` → `rustls` → `zeroize`/`idna_adapter` — requires `edition2024`, i.e. a modern toolchain). Consider adding a `rust-toolchain.toml` pinning a known-good version, and documenting the minimum version in `README.md`, so contributors on older distro-packaged Rust don't hit confusing `edition2024` errors.
  - [x] Done 16 Sep 2026 — `rust-toolchain.toml` pins channel `1.98.1` (+ rustfmt, clippy components); README "Prerequisites" now lists Rust 1.98+.

---

## Quick status snapshot (for your own tracking)

| Area | Status |
|---|---|
| Frontend (React/Vite/TS) | ✅ Builds clean, verified |
| Rust core — code hygiene | ✅ No stubs/`todo!()`/fake success paths found |
| Rust core — real hardware behavior | ❌ Unverified (P0 #2) — `HARDWARE_VALIDATION.md` run template ready |
| Windows secure-erase correctness | ✅ Fixed 16 Sep 2026 (P0 #1) — `AtaFlags` data-out `0x04` + byte-assert tests; real-device run still pending |
| macOS | ❌ Not supported (P1 #3) — decision logged: Windows+Linux only, loud runtime refusal |
| Forensic validation | ❌ Not done (P1 #4) — CFReDS harness added, corpus run pending |
| Security hardening | ❌ Not done (P1 #5) — `SECURITY_REVIEW.md` checklist ready, fuzz run pending |
| Signing scheme | ⚠️ Symmetric only (P1 #6) |
| Dashboard / packaging / docs | ❌ Not started (P2) |
| MSRV pin | ✅ Done 16 Sep 2026 — `rust-toolchain.toml` (1.98.1) + README note |
