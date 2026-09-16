# Purgent — Feature List

Complete feature inventory for **Purgent** (cross-platform forensic sanitization & recovery).
Stack: Rust core · Tauri v2 · React/TypeScript · SQLite.

---

## 1. Secure erasure engine

| Feature | Description |
| --- | --- |
| **Targeted erasure** | Real device targets (ATA / NVMe) and image files, chosen in-app. |
| **Operator confirmation** | The exact target phrase must be typed back before any destructive pass; a mismatch refuses the operation. |
| **No silent fallback** | If hardware erase fails, the operator must explicitly acknowledge an overwrite fallback — the reason and method are recorded in the report (`fallback_reason`). |
| **Read-back verification** | After each pass, sectors are re-read and compared to the expected pattern; mismatches are counted and persisted at operation time (never recomputed). |
| **Skipped-sector logging** | Bad sectors are logged per-LBA, skipped, counted, and reported — they never silently fail the wipe. |

### Wipe standards
| Standard | Type |
| --- | --- |
| NIST SP 800-88 Rev.1 — Clear | Single-pass overwrite (software) |
| NIST SP 800-88 Rev.1 — Purge | Hardware cryptographic / controller-level (in-band) |
| DoD 5220.22-M | Multi-pass overwrite + verify |
| IEEE 2883-2022 Purge | First-class flash/solid-state purge semantics |
| ISO/IEC 27037 Evidence Handling | Evidence-capture workflow (pre-wipe SHA-256) |
| ATA Secure Erase (in-band) | SATA drives, security feature set |
| NVMe Sanitize — Crypto Erase (in-band) | NVMe drives, controller-level erase |

---

## 2. Hardware / storage layer

| Feature | Description |
| --- | --- |
| **Device enumeration** | Windows (raw device discovery) + Linux (`/sys/class/block`) — lists model, serial, capacity, media type, bus, removable flag. |
| **ATA Secure Erase** | Real security-feature-set sequence (set password → unlock → erase-prepare → erase-unit) via `IOCTL_ATA_PASS_THROUGH` (Windows) and `HDIO_DRIVE_TASKFILE` (Linux). |
| **NVMe Sanitize** | NVMe admin Sanitize (crypto erase) issued to the controller on both platforms. |
| **HPA / DCO detection** | Parses ATA IDENTIFY DEVICE data; computes native vs. reported max LBA; detects Host Protected Area and Device Configuration Overlay. |
| **HPA / DCO removal (real, executed)** | Issues SET MAX ADDRESS (`0xF9`) / SET MAX ADDRESS EXT (`0x37`) and DCO RESET (`0xB1`) task files (Windows `AtaPassThroughEx`, Linux `HDIO_DRIVE_TASKFILE`), then re-queries IDENTIFY to verify restoration. Non-data commands — they never touch user data. |
| **48-bit LBA support** | LBA48 addressing with HOB (previous) register handling for >128 GiB media. |

---

## 3. File eraser

| Feature | Description |
| --- | --- |
| **Secure file deletion** | Multi-pass overwrite → read-back verify → delete-only-if-verified. |
| **Slack-space scrubbing** | Final-sector slack space overwritten (best-effort). |
| **Trace scrubber** | Shell history, Trash purge (Linux), thumbnail-cache note, slack scrub — each action logged honestly as `Ok` / `BestEffort` / `Skipped`. |
| **Exclusive-open refusal** | In-use files are refused rather than partially erased. |
| **File erase standards** | NIST SP 800-88 Rev.1 Clear (1 pass) · Purge (3 pass: zeros, 0xFF, random). |
| **Folder erase** | Recursively erases all contained files. |

---

## 4. Recovery (file carving)

| Feature | Description |
| --- | --- |
| **Signature carving** | Recovers 8 formats: JPEG, PNG, GIF, BMP, PDF, ZIP, **MP4** (moov-atom aware), **DOCX** (ZIP-EOCD aware, distinct from generic ZIP). |
| **Structure validation** | Per-type checks: JPEG segment walk, PNG CRC-32 chunk walk, ZIP EOCD check, PDF object/EOF presence, MP4 moov-atom check. |
| **Fragmented reconstruction** | Bifragment gap carving (implemented & tested for JPEG). |
| **Confidence scoring** | Per-factor breakdown: structure validity, terminator presence, fragment reconstruction, size consistency. |
| **Classification** | Rule-based `FileCategory` tagging + category aggregation per case. |
| **Read-only source guarantee** | The recovery source is opened under read-only semantics — enforced at the API level, not just a UI toggle. |
| **Extraction hashing** | Every recovered file is SHA-256 hashed at extraction time. |

---

## 5. Evidence & reporting

| Feature | Description |
| --- | --- |
| **Signed reports** | HMAC-SHA256 over a canonical (sorted-key) JSON payload, per-install signing key. |
| **Offline-first ledger** | Local SQLite case database (`CaseDb`) that survives app restart — fully air-gapped. |
| **Report verification** | Every stored report is revalidated on load; verified vs INVALID status is shown per row. |
| **Export formats** | PDF (human-readable) + JSON + XML (machine-readable) per operation. |
| **Operator identity** | Persisted per install, attached to every operation; fingerprint exposed for chain-of-custody. |
| **Compliance matrix** | Report × operation × verification × mismatch × signature × hash grid in the UI. |
| **Storage matrix** | Attached-device table (bus, media, capacity, removable, health). |

---

## 6. Cloud sync (optional)

| Feature | Description |
| --- | --- |
| **Supabase mirror** | Opt-in REST sync of report **metadata** only — never recovered file contents. |
| **Environment-gated** | Enabled only when `PURGENT_SUPABASE_URL` + `PURGENT_SUPABASE_ANON_KEY` are set. |
| **Offline-first** | Network down → local operation/report still completes; retry + dedup on reconnect. |
| **No telemetry** | No background pings; sync only runs on explicit operator action / toggle. |

---

## 7. Platform & engineering

| Feature | Description |
| --- | --- |
| **Cross-platform** | Windows (x64) primary (native `windows-sys` ioctls); Linux (x64) via Ubuntu CI re-verified on every push. |
| **Architecture** | `purgent-core` Rust engine is GUI-independent and separately testable; Tauri v2 shell + React UI. |
| **CI** | `.github/workflows/ci.yml` runs tests, formatting, and the frontend build on each push. |
| **Security practices** | `zeroize` for sensitive buffers (e.g., ATA passwords); keys never committed. |

---

## 8. Standards compliance

| Standard | Coverage |
| --- | --- |
| **NIST SP 800-88 Rev.1** | Clear / Purge paths |
| **DoD 5220.22-M** | Overwrite variants handled natively |
| **IEEE 2883-2022** | First-class purge semantics, surfaced in reports |
| **ISO/IEC 27037** | Pre-wipe evidence hash + recovery workflow |

## 9. Roadmap status

| Phase | Status |
| --- | --- |
| 1 – Foundation & overwrite engine | ✅ Complete |
| 2 – Cross-platform storage layer | ✅ Complete |
| 3 – Live-device wiping | ✅ Complete |
| 4 – File erase + slack scrub | ✅ Complete |
| 5 – Carving & recovery | ✅ Complete |
| 6 – Classification | ✅ Complete |
| 7 – Signed reporting | ✅ Complete |
| 8 – PDF / XML export | ✅ Complete |
| 9 – Tauri desktop app | ✅ Complete |
| 10 – Verification & hardening | ✅ Complete |
| 11 – Optional Supabase sync | ✅ Complete |
| 12 – HPA / DCO query + real removal | ✅ Complete |
| 13–15 – Companion dashboard, hardening, CFReDS validation, packaging | 🔜 Planned |

> Not yet implemented (honestly tracked): Gutmann 35-pass mode, Ed25519 asymmetric signing,
> macOS support — all intentionally deferred by design.