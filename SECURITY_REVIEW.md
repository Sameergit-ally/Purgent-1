# Purgent — Security Review

Status tracker for advisory security hardening (gap-plan P1 #5 / `KNOWN_ISSUES.md` KO-3).
Keeping a deviation here does not close an item — a closed item requires the executed evidence
described below.

## In-scope threat surface

- ATA / NVMe pass-through command builders (`storage/secure_erase.rs`, `storage/hpa_dco.rs`)
- Device / path selectors entering from the UI → IPC layer (`src-tauri/src/lib.rs` commands)
- Operator-confirmation race (enumeration vs execution target drift)
- Carving input parsing (malformed header bytes)
- Signing/verification handling (see KO-4, tracked separately)

## Checklist (P1 #5 fix steps)

- [ ] Fuzz the ATA/NVMe command-builder functions for panics / UB on malformed input.
      Planned targets (`cargo-fuzz`, nightly):
      - `ata_task_file_builder(bytes: &[u8])` — 8-byte task file → APT struct fields
      - `nvme_admin_cmd_builder` — 40-byte CDW region → nvme_admin_cmd
      - `parse_identify(&[u8])` — 512-byte IDENTIFY payload (word boundary math)
      Run requirement: a meaningful duration (e.g. several minutes / ≥ 1M iterations) with
      zero crashes, recorded here.
- [ ] Audit every device-path / file-path IPC entry point for traversal & injection:
      - `list_devices` (enumeration only — targets must be re-read by id before erase)
      - `erase_path` (device/image, operator-confirmed exact string)
      - `erase-file` path + `recover` source/output folder selectors
      Note existing guards: `verify_or_refuse_path` (non-regular/in-use refusal), carve source
      is opened read-only, recovery output dir is created fresh.
- [ ] Confirm erase targets the **same** device the operator confirmed (re-open by id, compare
      serial/model) — no silent re-resolution drift between confirmation and execution.
- [ ] Write findings and dispositions into this file; triage anything found (fix or explicit accept).

## Findings

*(none recorded yet — this becomes the audit trail as each checklist item is executed.)*