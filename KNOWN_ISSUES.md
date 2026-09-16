# Purgent — Known Issues

Tracking rule (see `RULES.md` §5): any correctness issue found in a security-relevant path
(wiping, hashing, signing, write-blocking, ATA/NVMe pass-through, HPA/DCO) must be logged here
(effectively an open item in `DECISIONS.md` / `Purgent_Gaps_Action_Plan.md`) until it is fixed
*and* re-validated. Entries are closed only when the fix is evidence-backed, not merely written.

---

## Open

| ID | Area | Issue | Status / precondition |
| --- | --- | --- | --- |
| KO-1 | Live hardware (P0 #2) | ATA Secure Erase, NVMe Sanitize, HPA/DCO removal are compile-validated only — never run against a real device. | Open — needs a disposable physical drive on Windows and Linux; no evidence yet |
| KO-2 | Forensic validation (P1 #4) | Carving/carving recovery validated only against synthetic test files, not a recognized forensic corpus (NIST CFReDS). | Open — harness added (`recovery.rs` env-gated test), corpus run pending |
| KO-3 | Security hardening (P1 #5) | No fuzzing of ATA/NVMe command builders, no published path-selector/IPC audit. | Open — see `SECURITY_REVIEW.md`; fuzz targets need nightly `cargo-fuzz` |
| KO-4 | Signing (P1 #6) | HMAC-SHA256 only (per-install symmetric key). Adequate for single-install chain-of-custody; no multi-party non-repudiation. | Open — Ed25519 on roadmap; limitation disclosed in README / reports |
| KO-5 | macOS (P1 #3) | Deliberately unsupported. Hardware paths return explicit `Unsupported` errors at runtime (no silent fallthrough). | Resolved by decision — see `DECISIONS.md` [2026-09-16] |

## Fixed

| ID | Area | Issue | Evidence |
| --- | --- | --- | --- |
| KO-0 | Windows ATA Secure Erase (P0 #1) | `AtaFlags` for the data-out branch in `secure_erase.rs` used `0x40`; correct WDK constant is `ATA_FLAGS_DATA_OUT = 0x04`. Wrong data-direction flag could make real-device data-out commands fail/hang. | Fixed in `secure_erase.rs` — `ata_flags(true) → ATA_FLAGS_DATA_OUT (0x04)`, `false → 0x00`; constants centralised in `hpa_dco.rs` and asserted by byte-value tests `ata_flags_matches_wdk_constants` + `ata_task_header_serializes_wdk_data_out_flag`; `cargo test --workspace` 82 passed. See `DECISIONS.md` |
| KO-6 | Verification honesty (Phase-1 rewrite) | Erase/wipe verification previously counted a failing/partial/short-read tail or a partially-corrupted sector as clean, and `Passed` reports could be emitted without a fully-clean read-back. | Fixed (evidence-backed, 5 files: `drive_eraser.rs`, `file_eraser.rs`, `persistence.rs`, `reporting.rs`, `trace_scrubber.rs`): `verify_pass` now compares **every** read-back byte including the partial tail sector, tallies `bytes_verified`/`mismatched_sectors`/`skipped_sectors` honestly, and `Passed` requires `mismatched==0 && bytes_verified==capacity && skipped.is_empty()`. Files are deleted **only after** a fully-clean verification (`drop(file)` before `remove_file` on Windows). `cargo test --manifest-path crates/purgent-core/Cargo.toml --lib` → **84 passed, 0 failed, 1 ignored** (incl. `verification_detects_corrupted_partial_tail_sector`, `single_file_nist_clear_erases_verifies_and_deletes`, `erase_operation_emits_signed_report`); `cargo fmt --check` ×2 clean; `cargo clippy --all-targets` 0 errors; `cargo build` success. See `DECISIONS.md` |

## Non-security / roadmap items

Home for P2 roadmap items (see also `Purgent_Gaps_Action_Plan.md`): read-only web dashboard,
native installers (MSI/NSIS/`.deb`/AppImage), end-user manual, deep architecture docs,
performance benchmarks, Gutmann 35-pass (deferred by decision). None of these affect the
security claims above; they are tracked in the gap plan.

## Honestly deferred (not claim-to-fix — see `DECISIONS.md` [Linux-only / runtime-unverifiable on this host])

1. **Linux-only atomic shell-history + exact-match trace scrub** (`trace_scrubber.rs`): the
   non-test Linux compile break was fixed (removed `test` from the `#[cfg]` gate), but atomic
   rewrite + history exact-match are Linux-targeted and cannot be compile-/test-verified on
   this Windows-only host. Recorded here, **not** claimed done; tracked as open in
   `Purgent_Gaps_Action_Plan.md` / `DECISIONS.md`.
2. **Frontend status honesty (`src/App.tsx` "VERIFIED SECURE" chip + CSP)**: the chip currently
   implies verified whenever there's no error. Phase-1 already makes success **only** signal a
   fully-verified+deleted run (the honest precondition), so the chip's signal is honest by
   construction. Any further chip/CSP change requires a live WebView runtime (npm build is
   compile-only here), so it is deferred — tracked, not blind-coded. See `DECISIONS.md`.