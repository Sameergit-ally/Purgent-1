# Purgent — User Guide (Step-by-Step)

How to install, run, and use every feature of Purgent, in numbered steps.

---

## 1. Run the project

### Prerequisites
- **Node.js 20+** and **npm**
- **Rust** toolchain (stable)
- **Windows:** MSVC C++ Build Tools ("Desktop development with C++") + WebView2 Runtime
- **Linux:** GTK / WebKit dependencies — see the [Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/)

### Step 1 — Install dependencies
```
npm install
```

### Step 2A — Run the desktop app (full functionality)
```
npm run tauri dev
```
Builds the Rust engine and opens the native desktop window.

### Step 2B — Run the browser preview only (no hardware access)
```
npm run dev
```
Opens Vite at `http://localhost:1420`. The browser preview **cannot** issue device ioctls —
hardware wipe/HPA-DCO features need the desktop shell. Use it only to preview the UI.

### Step 3 — Build & test (optional)
```
npm run build              # typecheck + production frontend build
cargo test --workspace     # run the engine test suite
npm run tauri build        # production desktop installer
```

---

## 2. Set your operator identity

Every operation is recorded under an operator ID so your reports are attributable.

1. Open the desktop console (click **Launch Desktop Console** on the landing page).
2. In the **Operator Identity** box, type your operator ID (e.g. `DFIR-OPR-40291`).
3. Click **Set identity**.
4. Your fingerprint and audit vault path appear under **Fingerprint / Audit Vault**.

> The app auto-loads your saved identity on next launch.

---

## 3. Secure Erase — drive or image file

### 3A. Erase an image file (file target)
1. In the console, open the **Secure Erase** tab.
2. Set **Target type** to `Image file (recommended)`.
3. In **Path to disk image**, enter the full path, e.g. `C:\cases\disk.img`.
4. Choose a **Standard** (hardware methods are automatically hidden for file targets):
   - NIST SP 800-88 Rev.1 Clear
   - NIST SP 800-88 Rev.1 Purge
   - DoD 5220.22-M
   - IEEE 2883-2022 Purge
   - ISO/IEC 27037 Evidence Handling
5. In **Confirm target — type exactly**, type the exact phrase shown above the box.
6. Click **Begin secure erase**.
7. Watch progress in the **Progress Monitor**; when done, the report appears in
   **Reports & Compliance**.

### 3B. Erase a physical device (admin required)
1. In the console, open the **Secure Erase** tab.
2. Set **Target type** to `Physical device (admin required)`.
3. Pick your drive from the **Device** dropdown. Verify the model/serial/capacity carefully —
   this wipes **every sector**.
4. Choose a **Standard**:
   - **ATA Secure Erase (in-band)** — SATA drives
   - **NVMe Sanitize — Crypto Erase (in-band)** — NVMe drives
   - Any overwrite standard (NIST Clear / Purge, DoD 5220.22-M, IEEE 2883-2022, ISO 27037)
5. Type the **exact** confirmation phrase (device id + capacity).
6. Click **Begin secure erase**.

> **Fallback acknowledgement:** if the hardware can't honor the command, Purgent **never**
> silently downgrades. A dialog asks you to either:
> - **"I acknowledge the overwrite fallback"** — continues with a verified pseudo-random overwrite
>   (the report records the method + reason), or
> - **"Cancel — do not attempt fallback"** — aborts with no change to the drive.

### 3C. HPA / DCO removal (automatic on device wipes)
During a device wipe, Purgent automatically:
1. Reads the ATA IDENTIFY data (`storage/hpa_dco.rs`).
2. Detects Host Protected Area (HPA) / Device Configuration Overlay (DCO).
3. Issues **SET MAX ADDRESS** (0xF9 / EXT 0x37) and **DCO RESET** (0xB1) task files.
4. Re-queries IDENTIFY to verify native capacity is restored.

To confirm it happened, open the report and check the **hpa/dco removal** line:
`sent_hpa=true · sent_dco=true · removed=true`.

---

## 4. Erase files / folders

1. Open the **Erase Files** tab.
2. Set **Target type** to `Single file` or `Folder (all contained files)`.
3. Enter the **Path**, e.g. `C:\cases\file.pdf` or `C:\cases\folder`.
4. Choose a **Standard**:
   - NIST SP 800-88 Rev.1 Clear (1 pass)
   - Purge (3 pass): zeros, 0xFF, random
5. Type the **exact** confirmation phrase.
6. Click **Begin file erase**.

> Each file is overwritten, **verified**, and only then deleted. The file's final-sector slack
> space is scrubbed (best-effort), and metadata/trace scrub actions are honestly logged
> (`Ok` / `BestEffort` / `Skipped`) in the report.

---

## 5. Recovery (file carving)

1. Open the **Recovery** tab.
2. In **Source (read-only)**, enter the disk image or drive path to scan —
   e.g. `C:\cases\source.img`. The source is opened read-only.
3. In **Output folder**, enter where recovered files should be saved —
   e.g. `C:\cases\recovered`.
4. Click **Begin recovery**.
5. Purgent scans for signatures (JPEG, PNG, GIF, BMP, PDF, MP4, DOCX, ZIP), validates structure,
   reconstructs fragmented files, and scores confidence.
6. Open **Reports & Compliance** to see the recovered files, categories, and per-file SHA-256 hashes.

---

## 6. Reports & Compliance (evidence review)

1. Open the **Reports & Compliance** tab.
2. The **Certificate Vault** table lists every operation with Status / Type / Target / Standard /
   Operator / Started.
   - `verified` = valid HMAC-SHA256 signature · `INVALID` = tampered.
3. Click **view** on a row to open the full report:
   - Check **method**, **evidence sha256**, **hpa/dco**, **hpa/dco removal**, **trace scrub**,
     **categories**, verification status + mismatched-sector count.
   - The signed payload is shown as JSON at the bottom.
4. Click **PDF** next to a row (or **Open PDF certificate**) to open the PDF report.
5. Click **Export XML certificate** to save the machine-readable XML; **Open XML certificate** to
   view it.
6. **Compliance Matrix · LEGAL GRID**: report × operation × verification × mismatch × signature ×
   hash — click a row to populate the detail columns.
7. **Attached Storage & Write-Block Matrix**: lists detected devices, bus, media, capacity,
   removable status.

---

## 7. Cloud sync (optional, Supabase)

Sync mirrors report **metadata only** (never file contents) to Supabase.

1. Set the environment variables:
   - `PURGENT_SUPABASE_URL`
   - `PURGENT_SUPABASE_ANON_KEY`
2. In **Reports & Compliance**, find the **Cloud sync (Supabase)** panel.
3. Click **Enable sync** (once configured). A green `sync enabled` badge appears.
4. Click **Sync now** to push pending report metadata immediately.
5. Offline-first: if the network is down, local operations and reports complete normally; sync
   retries later.

---

## 8. CLI mode (no UI)

### List detected devices
```
cargo run --example list_devices
```

### Wipe a temporary image end-to-end (safe demo)
```
cargo run --example wipe_demo <image-file> <standard> <operator-id>
```
Where `<standard>` is one of: `nist_clear` · `nist_purge` · `dod_3pass`.

Example:
```
cargo run --example wipe_demo C:\temp\test.img nist_purge DFIR-OPR-40291
```
The CLI prints the exact target description — type it back to continue, or abort.

> Always test on a throwaway image file first. Never point destructive operations at a drive
> whose contents you want to keep.

---

## Safety rules (from RULES.md)
- You must have the legal right to erase/analyze the target.
- Operator confirmation must **exactly match** the target phrase.
- No silent fallback: hardware erase failure always requires explicit acknowledgement.
- No scheduled/unattended wipes, no auto-confirm.