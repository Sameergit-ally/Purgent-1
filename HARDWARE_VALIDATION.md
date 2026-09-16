# Purgent — Hardware Validation Log

Live-hardware evidence for ATA Secure Erase / NVMe Sanitize / HPA/DCO removal (see `RULES.md` §7).

**Rule:** a compile/test-green run is **not** evidence of real-device correctness. This log is the
only place a hardware method may be marked "verified." No entry here = the method stays
**unverified** in `KNOWN_ISSUES.md` (KO-1) and `Purgent_Gaps_Action_Plan.md` (P0 #2).

## Safety contract

- Use **only** disposable/scratch media — never a drive containing anything of value.
- Each run is explicitly authorized by a human (self-owned or contracted sanitization).
- Writes down the exact commands issued and the before/after `IDENTIFY DEVICE` state.
- If a run fails or behaves unexpectedly, record it here too — a recorded failure beats an
  unrecorded assumption.

## Run template

```markdown
### Run #N — <method>   (ATA Secure Erase | NVMe Sanitize | HPA removal | DCO removal)
- **Date (UTC):** YYYY-MM-DDThh:mm:ssZ
- **Operator:** <operator-id / initials>
- **OS + kernel/build:** e.g. Windows 11 24H2 build 26100 / Ubuntu 24.04 kernel 6.8
- **Device:** model + firmware + serial (e.g. Samsung 870 EVO SATA 2TB)
- **Connection:** e.g. native SATA port / NVMe M.2 / USB bridge (USB bridges may block commands)
- **Admin mode?:** elevated shell (root / Administrator)
- **Exact command issued:** (copy the log line, e.g. `operation=... method=ata_secure_erase`)
- **Before `IDENTIFY DEVICE`:** model word 27…46, capacity words 60/61 or 100…103, security
  feature set (word 128), HPA/DCO state
- **After `IDENTIFY DEVICE`:** re-queried values
- **Verify method:** read-back pattern / hex dump sample / controller status
- **HPA/DCO:** artificially set HPA via `hdparm -N` (Linux) — records capacity removed then restored
- **Result:** PASS | FAIL | behavior note
- **Evidence attachments:** logs / screenshots / hexdumps saved next to this file
```

## Entries

*(none yet — add runs above, one per method, on Windows and Linux, before claiming any of these
as verified.)*