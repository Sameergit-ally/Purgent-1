# Purgent — Usage Guide (Step-by-Step)

Agar tumne app kholna hai, sabse pehle ye 2 maamle karo:

```sh
npm run tauri dev
```

App ka desktop window khul jayega. Landing page par click karo: **Launch Desktop Console**.
*Hint: yahi app ka main console hai — yahan sab features chalenge.*

---

## Feature 1 — Drive / Disk Wipe (Secure Erase)

Ek pure drive ko permanently wipe karne ke liye.

- **Step 1:** Console mein **Secure Erase** tab kholo.
  *Hint: top par 4 tabs hain — Secure Erase, Erase Files, Recovery, Reports.*

- **Step 2:** Target type choose karo — `Physical device (admin required)` ya `Image file (recommended)`.
  *Hint: real drive ke liye "Physical device", test file ke liye "Image file".*

- **Step 3:** (Device wala) Device dropdown se apni drive choose karo.
  *Hint: model/serial/capacity dhyan se dekh lo — is drive ka poora data wipe hoga, laut kar nahi aayega.*

- **Step 4:** Standard choose karo:
  - `ATA Secure Erase (in-band)` → SATA drive ke liye
  - `NVMe Sanitize — Crypto Erase (in-band)` → NVMe drive ke liye
  - ya overwrite wale: `NIST SP 800-88 Clear`, `DoD 5220.22-M`, etc.
  *Hint: image file target hone par hardware standards chhup jate hain — woh sirf real devices ke liye hain.*

- **Step 5:** "Confirm target — type exactly" wale box mein wahi phrase type karo jo neeche dikh raha hai.
  *Hint: exact copy type karo — galti pe operation refuse ho jayega (ye security feature hai).*

- **Step 6:** **Begin secure erase** par click karo.
  *Hint: Progress Monitor mein live progress dikhega.*

- **Step 7:** Agar hardware erase fail ho (kabhi kabhi drive command nahi maanti) — ek dialog aayega:
  - **"I acknowledge the overwrite fallback"** → overwrite se wipe hoga (report mein reason record hoga)
  - **"Cancel"** → drive bilkul chhu nahi jayega
  *Hint: Purgent kabhi silently fallback nahi karta — tumhari permission zaroori hai.*

---

## Feature 2 — File / Folder Secure Delete

Ek file ya folder ko destroy nasha (overwrite + verify + delete).

- **Step 1:** **Erase Files** tab kholo.

- **Step 2:** Target type choose karo — `Single file` ya `Folder (all contained files)`.
  *Hint: folder select karoge to andar ki saari files erase hongi.*

- **Step 3:** **Path** mein full path daalo, e.g. `C:\cases\oldfile.pdf`.
  *Hint: path sahi ho — galat file delete hogi to recover nahi ho paayegi.*

- **Step 4:** Standard choose karo:
  - `NIST SP 800-88 Rev.1 Clear (1 pass)`
  - `Purge (3 pass): zeros, 0xFF, random`
  *Hint: 3-pass zyada safe hai, 1-pass quick hai.*

- **Step 5:** Exact confirmation phrase type karo → **Begin file erase** click karo.

- **Step 6:** Result — file overwrite → read-back **verify** → phir **delete**.
  *Hint: agar file kisi program mein khuli hai (in-use), to erase refuse ho jayega — pehle usse band karo.*

---

## Feature 3 — Recover Deleted Files (Carving)

Purani deleted files ki disk se wapas nikaalna.

- **Step 1:** **Recovery** tab kholo.
  *Hint: recovery source hamesha read-only use hota hai — source pe kuch write nahi hota.*

- **Step 2:** **Source (read-only)** mein disk image / drive ka path daalo, e.g. `C:\cases\source.img`.

- **Step 3:** **Output folder** daalo jahan recovered files save honi hain, e.g. `C:\cases\recovered`.
  *Hint: output folder apne aap ban jayega.*

- **Step 4:** **Begin recovery** click karo.
  *Hint: scan mein time lag sakta hai — signatures dhundh ke files validate hongi.*

- **Step 5:** Recovered files **Reports & Compliance** mein dekhlo — har file ka SHA-256 hash ke saath.

---

## Feature 4 — Reports / Proof (PDF, XML)

Har wipe/recover ka signed proof dekhna.

- **Step 1:** **Reports & Compliance** tab kholo.
  *Hint: Certificate Vault table mein saari operations hain.*

- **Step 2:** Kisi row par **view** click karo — pura report kholta hai.
  *Hint: `verified` = signature theek hai, `INVALID` = report ke saath chhed-chaad.*

- **Step 3:** **PDF** open karo, ya **Export XML certificate** click karke machine-readable copy save karo.
  *Hint: ye report court/audit mein proof ke liye use hoti hai.*

---

## Optional — Cloud Sync (Supabase)

- Environment variables set karo: `PURGENT_SUPABASE_URL` + `PURGENT_SUPABASE_ANON_KEY`.
- Reports tab mein **Cloud sync (Supabase)** panel → **Enable sync** → **Sync now**.
  *Hint: sirf report ka metadata sync hota hai — recovered file contents kabhi upload nahi hote.*

---

## Safety Note

- Hamesha pehle **dummy image file** pe try karo, real drive pe nahi:
  ```sh
  cargo run --example wipe_demo C:\temp\test.img nist_clear DFIR-OPR-40291
  ```
- Jis drive/file ko erase kar rahe ho us par tumhara legal right hona chahiye.
- Koi scheduled/unattended wipe nahi, koi auto-confirm nahi — har destructive kaam pe confirmation zaroori hai.

*Hint: safe demo ke liye pehle ek chhoti saal image file bana lo (`12288` bytes, JPEG jaisa), phir us par wipe/recover try karo.*