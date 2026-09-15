# Purgent — Hackathon Prep Guide (Theory + Q&A)

Clear, real theory so you understand the project, not just memorize. Part 1 = theory + study
checklist, Part 2 = judge-style Q&A that covers the whole project in short form.

---

## Part 1 — THEORY (padhai ka saman)

### 1. Rust — the engine language

**Ownership & borrowing.** Every value has exactly one owner. When the owner goes out of scope the
value is dropped (no GC, no memory leaks). You can `&borrow` a value without taking ownership
(reads), or `&mut borrow` to change it — but only one mutable borrow at a time. This is why Rust
code with threads and raw disk I/O is safe by default.

```rust
fn erase_block(data: &mut [u8]) {          // borrow, not ownership
    data.fill(0x00);                        // mutating through &mut
}                                           // caller still owns the buffer
```

**`Result` / `Option`.** Rust has no exceptions. Functions that can fail return
`Result<T, E>` = `Ok(value)` or `Err(e)`. Functions that might have "no value" return
`Option<T>` = `Some(v)` or `None`. The `?` operator unwraps success and *returns the error early*
from the current function:

```rust
fn wipe(path: &str) -> Result<(), String> {
    let file = File::open(path)?;          // Err -> returns early
    Ok(())
}
```

In the engine, hardware failures are typed errors (`HardwareEraseError`, `WipeError`) so the UI
can distinguish "device busy" from "needs fallback acknowledgement".

**Enums + `match`.** Enums model "one of several" — used heavily for `WipeMethod`,
`WipeStandard`, `MediaType`. `match` forces exhaustive handling:

```rust
match media_type {
    MediaType::Ata => at::secure_erase(path),
    MediaType::Nvme => nvme::sanitize(path),
    MediaType::Unknown => Err("unsupported".into()),
}
```

**serde.** `#[derive(Serialize, Deserialize)]` turns structs into JSON/XML safely and back —
used for reports, requests, and persistence. `#[serde(default)]` fills missing JSON keys with
`Default::default()`.

**`#[cfg]` — conditional compilation.** The same crate compiles on Windows and Linux by picking
different code at build time:

```rust
#[cfg(windows)]          fn open_disk(p: &str) -> File { /* CreateFileW */ }
#[cfg(target_os = "linux")] fn open_disk(p: &str) -> File { /* open + ioctl */ }
```

CI builds the Linux version on Ubuntu on every push, so both paths stay tested.

### 2. Tauri v2 — the desktop shell

Tauri = Rust backend + *system webview* (WebView2/WebKit) frontend. Much lighter than Electron
and can run privileged native code safely.

**IPC (invoke).** JS calls Rust by name through `invoke`:

```rust
#[tauri::command]
fn wipe_device(path: String, standard: String, fallback_acknowledged: bool) -> Result<WipeResult, String> { ... }
```

```ts
const result = await invoke("wipe_device", { path, standard, fallback_acknowledged });
```

**Events (progress).** Long operations push progress from Rust to JS:

```rust
app.emit("wipe-progress", update);           // Rust side
listen("wipe-progress", (e) => setProgress(e.payload));   // JS side
```

**State.** `State<AppState>` holds live app data (signing key, DB path, operator id, sync config).

### 3. React 18 + TypeScript

- **Component/props/state:** UI is a tree of components; props flow down, state is local
  `useState`.
- **Hooks:** `useEffect` runs side effects (calls to Tauri, timers), `useMemo` caches heavy
  computations, `useCallback` stabilizes callbacks.
- **TypeScript:** interfaces like `interface Report { report_hash: string; ... }` catch mistakes
  at build time (`npm run build` runs `tsc`).
- The console screen calls `invoke()` and renders live wipe progress; the reports screen lists
  stored reports and shows signed/verified badges in the ReportView.

### 4. Vite + Tailwind

- Vite dev server serves the app at `http://localhost:1420` (`npm run dev`); `npm run build`
  typechecks + bundles into `dist/`.
- Tailwind = utility CSS classes (`flex`, `text-white`, `bg-[#0B111E]`). The app uses a dark
  "neomorphic" theme defined partly in `src/styles.css`.

### 5. SQLite (rusqlite)

A single-file embedded DB. `CaseDb` (in `persistence.rs`) stores signed reports locally so the
app is **offline-first**: every operation records its report locally before any optional sync.

### 6. OS-level disk access — the differentiator (study hard)

**Device access is via native OS APIs, not normal file I/O.**

- **Windows:** `CreateFileW("\\.\PhysicalDriveN", ...)` opens the raw disk. Then
  `DeviceIoControl(handle, IOCTL_ATA_PASS_THROUGH, ...)` sends an ATA command using a buffer laid
  out as `AtaPassThroughEx`:

```
Length(2) | AtaFlags(2) | PathId/TargetId/Lun/Reserved(4) |
DataTransferLength(4) | TimeOutValue(4) | ReservedAsUlong(4) |
DataBufferOffset(4) | PreviousTaskFile[8] | CurrentTaskFile[8]
```

- **Linux:** `open(path, O_RDWR | O_NONBLOCK)` then `ioctl(fd, HDIO_GET_IDENTITY, buf)` to read
  the 512-byte IDENTIFY block and `ioctl(fd, HDIO_DRIVE_TASKFILE, &task_request)` to send custom
  task files. `libc` doesn't export the HDIO_* numbers, so we pin them:
  `HDIO_GET_IDENTITY = 0x030d`, `HDIO_DRIVE_TASKFILE = 0x031d`.

**The task file (registers).** ATA commands are written into registers:

```
current = [features, sector_count, LBA_low, LBA_mid, LBA_high, device, status(out), command]
previous/HOB = [features_h, count_h, LBA24..31, LBA32..39, LBA40..47]   // 48-bit only
```

**LBA28 vs LBA48.** LBA28 addresses up to ~128 GiB and puts the top 4 LBA bits in the *device*
register (`0x40 | ((lba >> 24) & 0x0F)`). LBA48 extends addressing to ~128 PiB and uses the
**HOB (previous)** registers for the upper 24 bits.

**Key commands used by Purgent:**

| Command | Opcode | Purpose |
| --- | --- | --- |
| IDENTIFY DEVICE | 0xEC | read 512-byte capacity/feature data |
| SET MAX ADDRESS | 0xF9 | restore native capacity (HPA, 28-bit) |
| SET MAX ADDRESS EXT | 0x37 | same, 48-bit |
| DCO (SET DEVICE CONFIG) | 0xB1 / feat 0x04 | DCO reset |
| SECURITY SET PASSWORD | 0xF1 | begins ATA Secure Erase |
| SECURITY ERASE UNIT | 0xF4 | executes the erase |
| NVMe Sanitize | admin 0x84 (SANACT=2) | controller-level NVMe erase |

**Why HPA/DCO matter.** An HPA hides a region *beyond reported capacity*; a DCO can shrink what
the drive reports. Data in those regions survives OS-level wipes. We detect from IDENTIFY
(words 60/61 = current LBA28, 100–103 = native LBA48, word 162 bit0 = DCO feature), then *remove*
it before erasing, then re-query IDENTIFY to prove it.

**Why SSD over-write needs the controller.** Flash uses wear leveling + over-provisioning pools.
Logical address N doesn't map to a fixed NAND cell, so host writes never reach every cell — only
controller commands (Secure Erase / Sanitize) can clear the whole physical media.

### 7. Standards (pitch material)

- **NIST SP 800-88 Rev.1:** **Clear** (stop data from being recoverable by ordinary tools —
  overwrite/secure erase), **Purge** (resist state-of-the-art recovery — controller erase),
  **Destroy** (physical).
- **DoD 5220.22-M:** legacy multi-pass overwrite (passes of 0x00, 0xFF, pseudo-random, etc.).
- **IEEE 2883-2022:** modern alternative to 800-88; adds semantics like ATA/NVMe sanitize paths.
- **ISO/IEC 27037:** forensics — evidence capture, chain of custody, hashing before analysis.

### 8. Crypto / evidence

- **SHA-256:** one-way digest; we hash the raw medium *before* wiping (evidence capture) and hash
  canonical report JSON (`report_hash`).
- **HMAC-SHA256:** keyed digest; `sign_report` signs the canonical payload with the operator key.
  `verify_report` recomputes and compares — that's the tamper-evidence.
- `zeroize` wipes sensitive buffers (e.g. ATA passwords) from memory when done.

### 9. Networking / sync (optional)

- HTTP + REST + JSON basics; `ureq` is the HTTP client used.
- Supabase sync is **opt-in** (env `PURGENT_SUPABASE_URL` / `_ANON_KEY`), **metadata-only**
  (report rows, hashes, HMAC — never recovered file bytes), with retry + dedup, and it never
  blocks local operation.

### 10. Git & GitHub

- `git add/commit/push`, branches, PRs. README + Markdown for pitch docs. CI runs Ubuntu tests
  on every push so Linux stays green.

---

## Part 1b — Study Checklist (tick as you go)

- [ ] Rust: ownership, Result/Option/`?`, enums+match, serde, `#[cfg]`, cargo test
- [ ] Tauri: commands, invoke, events, State, tauri.conf.json
- [ ] React: props/state/hooks, TS types, ReportView + console screens
- [ ] Vite/Tailwind: `npm run dev`, port 1420, theming
- [ ] SQLite: CaseDb offline-first storage
- [ ] Windows: CreateFileW, DeviceIoControl, AtaPassThroughEx layout
- [ ] Linux: raw open, HDIO ioctls, task file structure
- [ ] ATA: IDENTIFY words, LBA28/LBA48, HOB regs, SET MAX, DCO reset
- [ ] NVMe: Sanitize admin command, controller vs namespace
- [ ] Standards: NIST 800-88 / DoD / IEEE 2883 / ISO 27037
- [ ] Crypto: SHA-256, HMAC, zeroize, read-back verification
- [ ] Sync: opt-in metadata-only Supabase mirroring
- [ ] Git/GitHub + README/pitch

---

## Part 2 — Judge-Style Q&A (short answers)

### Q1. What is Purgent?
**A.** A cross-platform desktop app that does standards-compliant secure erasure of drives/files
and forensic-grade recovery of deleted data — with every operation recorded in a tamper-evident,
signed audit report.

### Q2. What tech stack and why?
**A.** Rust core (safe, native, no GC) → Tauri v2 shell (system webview) → React/TypeScript UI →
SQLite (offline) + opt-in Supabase REST. OS-level ATA/NVMe ioctls require native code, so Rust was
the natural fit; `zeroize` keeps secrets safe.

### Q3. Why not Electron?
**A.** Electron bundles Node/Chromium — huge, RAM-hungry, and it can't safely issue raw disk
ioctls. Tauri is much smaller, uses the system webview, and keeps all privileged device access in
Rust.

### Q4. Why isn't `dd` / a normal format enough for an SSD?
**A.** SSDs use wear leveling and over-provisioned NAND pools; host writes only touch logical
sectors, so cells may retain old data. You must issue controller-level ATA Secure Erase / NVMe
Sanitize and then verify by read-back.

### Q5. What are HPA and DCO and why dangerous?
**A.** HPA hides a region beyond reported capacity; DCO restricts the drive's reported feature
set. Hidden data survives OS wipes. We detect via IDENTIFY and actively remove them before erasing.

### Q6. How does HPA/DCO removal actually work?
**A.** Parse IDENTIFY (words 60/61 vs 100–103 for capacity delta, word 162 for DCO), then issue
SET MAX ADDRESS (0xF9) or SET MAX ADDRESS EXT (0x37) plus DCO Reset (0xB1/feat 0x04), then
re-run IDENTIFY to prove capacity is restored.

### Q7. How do you talk to the drive?
**A.** ATA pass-through. Windows: `IOCTL_ATA_PASS_THROUGH` with an `AtaPassThroughEx` buffer.
Linux: `HDIO_DRIVE_TASKFILE` ioctl. We send the register blocks including 48-bit HOB registers for
LBA48 commands.

### Q8. Which standards do you comply with?
**A.** NIST SP 800-88 (Clear/Purge), DoD 5220.22-M, IEEE 2883-2022, and ISO/IEC 27037 for evidence
handling.

### Q9. How do you prove the wipe happened?
**A.** Each report records method, capacity, timestamps, read-back verification (mismatch count),
a pre-wipe SHA-256 evidence hash, and an HMAC-SHA256 signature over the canonical payload —
verifiable offline at any time.

### Q10. Describe your verification loops.
**A.** Overwrite: after each pass, read back and compare against the expected pattern, logging
mismatches per LBA. Hardware erase: issue the command, check the device/controller status, and
record the signed outcome. Skipped sectors are logged per-LBA.

### Q11. Is this scalable to big disks?
**A.** Yes — the engine streams in 1 MiB blocks (O(n), constant memory), so any capacity runs
without loading the disk into RAM.

### Q12. What's the security model?
**A.** Operator confirmation must match the target exactly before any destructive pass; no silent
fallback (hardware-erase failure requires explicit ack of an overwrite fallback); ATA passwords are
zeroized; the app works fully air-gapped with no telemetry.

### Q13. Is HPA/DCO removal data-destructive?
**A.** No. SET MAX and DCO Reset only change the addressing window (non-data commands). The
destructive step is the erase issued afterwards — which is the point of the tool.

### Q14. What about the sync / cloud?
**A.** Opt-in and metadata-only: report rows, hashes, HMAC. Recovered file contents are never
uploaded. If the network is down, the local operation and report still complete.

### Q15. Hardest part?
**A.** the ATA/NVMe layer: register layouts, LBA48 HOB handling, missing `libc` HDIO constants,
matching task-files to kernel semantics, and keeping both Windows and Linux (Ubuntu CI) green.

### Q16. Roadmap / next?
**A.** (Pick real ones: NVMe Format NVM variation + security-unlock polish, encrypted report
bundles, firmware fingerprinting, more destructive-path tests, production Supabase deploy docs.)

### Q17. How is this different from DBAN/shred?
**A.** They mostly can't reach controller cells, don't remove HPA/DCO, and produce no verifiable
audit. Purgent combines controller-level erase, hidden-area removal, read-back verification, and
signed forensic reports in one cross-platform app.

### Q18. Who's the target user?
**A.** Security/IT teams and forensic examiners who must prove media was sanitized to standards —
enterprise, gov/military air-gapped environments, and incident-response units.

---

## Pro tip for demo day
1. Demo on *your* laptop with the desktop app open and one wipe already verified.
2. One-line hook: **"Most 'wipes' don't reach the SSD's cells — ours does, and it proves it."**
3. Point judges to real code (`hpa_dco.rs` for removal, `secure_erase.rs` for ATA/NVMe, 
   `reporting.rs` for signatures) — knowing where things live beats memorizing.