# Purgent — Forensic Validation Log

Recovery/carving accuracy evidence against a **recognized public forensic dataset** (NIST CFReDS
or an equivalent with cataloged deleted files), per `RULES.md` §7.

**Rule:** until an entry exists below, "tested" means *tested against synthetic files only* — that
distinction must never be collapsed in reports or README claims. Absence of an entry => `KNOWN_ISSUES.md`
(KO-2) stays open.

## How to produce an entry

1. Download a corpus image with known, cataloged deleted files.
   - Example: NIST CFReDS (http://cfreds.nist.gov) `E01`/image or a smaller public carve corpus.
2. Convert/extract a raw image the engine can read (carve source is opened read-only, raw bytes).
3. Run the ignored harness against it:

   ```sh
   # optional manifest: one expected SHA-256 per line (ground truth of the corpus' cataloged files)
   set PURGENT_CFREDS_MANIFEST=C:\forensic\manifest.sha256   # leave unset if no catalog yet
   set PURGENT_CFREDS_IMAGE=C:\forensic\corpus.img
   cargo test -p purgent-core --release -- --ignored carve_against_cfreds_image --nocapture
   ```

4. Capture the printed summary (scanned bytes, recovered count, precision/recall vs manifest if
   provided) and paste it into an entry below.
5. Keep note of which category signatures were exercised (jpeg, png, gif, bmp, pdf, mp4, docx, zip)
   and whether fragmented reconstruction was covered.

## Entry template

```markdown
### Run #N — <dataset name>
- **Date (UTC):** YYYY-MM-DDThh:mm:ssZ
- **Dataset:** name / version / download source / license terms
- **Image:** raw size, offsets of cataloged deleted files if published
- **Command:** `cargo test -p purgent-core --release -- --ignored carve_against_cfreds_image`
- **Engine:** commit/version + standard config
- **Result (printed summary):**
  ```text
  [cfreds] scan summary: source=... scanned_bytes=... recovered_files=...
  [cfreds] precision=... recall=... tp=... fp=... fn=...
  ```
- **Per-signature breakdown:** expected vs recovered per file type
- **Fragmentation cases covered:** yes/no, gap sizes, reconstruction accuracy
- **Conclusion:** e.g. "overall recall 0.9x; high bmp false positives due to header-only carves"
```

## Entries

*(none yet — add a run above before claiming corpus-validated recovery.)*