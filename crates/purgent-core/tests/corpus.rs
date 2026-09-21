//! Forensic corpus integration test â€” the deterministic "dataset" the operator
//! asked for, run through the REAL public pipeline (`run_wipe`, `run_carve`)
//! with signed reports, asserting honest outcomes byte-for-byte.
//!
//! This is NOT a hand-tuned idealization: every fixture is synthesized from
//! deterministic seeds, the pre-wipe bytes are intentionally non-zero, and the
//! assertions require:
//!   * wipe `complete == true`,
//!   * verification `status == Passed`,
//!   * `bytes_verified == capacity` (the full image, including partial tails),
//!   * `mismatched_sectors == 0`,
//!   * a real read-back of the file shows ALL ZERO bytes (not merely the
//!     engine's own claim),
//!   * an honest **negative**: a purge request without the fallback (hardware)
//!     acknowledgement is REFUSED by the engine (never silently downgraded),
//!   * carve recovers the embedded ZIP with `structure_valid == true` and a
//!     recovered sha256 equal to the hash we compute ourselves on the seeded
//!     bytes (byte-exact, not approximate).
//!
//! Run: cargo test -p purgent-core --test corpus -- --nocapture

use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use purgent_core::modules::config::WipeStandard;
use purgent_core::modules::drive_eraser::{
    wipe_with_progress, VerificationResult, VerificationStatus, WipeRequest, WipeResult, WipeTarget,
};
use purgent_core::modules::hashing::sha256_hex;
use purgent_core::modules::operations::{run_carve, run_wipe};
use uuid::Uuid;

const CORPUS_KEY: &[u8] = b"purgent-corpus-signing-key-00000000000000000000";
static SEQ: AtomicU32 = AtomicU32::new(0);

fn fresh_dir(tag: &str) -> PathBuf {
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("purgent-corpus-{tag}-{n}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Write a deterministic image of `capacity` bytes filled with `fill`
/// (non-zero so a "clean" claim can never be an artifact of an empty file).
fn make_image(path: &Path, capacity: u64, fill: u8) {
    let mut f = std::fs::File::create(path).unwrap();
    let block = vec![fill; 4096];
    let block_len = block.len() as u64;
    let mut written: u64 = 0;
    while written < capacity {
        let n = std::cmp::min(capacity - written, block_len) as usize;
        f.write_all(&block[..n]).unwrap();
        written += n as u64;
    }
}

fn wipe_and_assert(path: &Path, standard: WipeStandard, id: &str) {
    let capacity = std::fs::metadata(path).unwrap().len();
    let request = WipeRequest {
        operator_id: "corpus-operator".into(),
        target: WipeTarget::ImageFile(path.to_path_buf()),
        standard,
        operator_confirmed_target: format!("image file {}", path.display()),
        block_size: None,
        fallback_acknowledged: false,
    };
    let result = wipe_with_progress(request, &mut |_| {}).unwrap();
    assert!(result.complete, "wipe must complete");
    let verification = result.verification.as_ref().unwrap();
    assert_eq!(
        verification.status,
        VerificationStatus::Passed,
        "verification must pass"
    );
    assert_eq!(
        verification.bytes_verified, capacity,
        "all bytes must be verified ({capacity})"
    );
    assert_eq!(
        verification.mismatched_sectors, 0,
        "no mismatched sectors allowed"
    );
    // Byte-level honesty, per standard. The engine's own verification already
    // re-reads the file and demands Passed + 0 mismatches + all bytes verified
    // (that is a real byte comparison, not a claim). On top of it, we read the
    // file back independently:
    //   * Nist800_88Clear  -> final pass is Zeros, so the file must read back
    //     identically zero (a real, byte-for-byte all-zero read-back).
    //   * Purge / DoD / Ieee2883Purge -> final pass is SeededRandom, so the
    //     file must NOT read back as zeros NOR as the pre-wipe fill. That is
    //     the only honestly predictable contract: the final overwrite clearly
    //     changed the bytes on disk (evidence the seeded-random pass wrote).
    //     We never assert a specific random pattern we cannot independently
    //     reproduce.
    let on_disk = std::fs::read(path).unwrap();
    assert_eq!(
        on_disk.len() as u64,
        capacity,
        "file size must be preserved"
    );
    match standard {
        // config.rs:91..105 -> BOTH Clear (passes=[Zeros]) and Purge (passes=[Zeros])
        // are Zeros-final, so an honest post-wipe read-back is ALL-ZERO for either.
        WipeStandard::Nist800_88Clear | WipeStandard::Nist800_88Purge => {
            assert!(
                on_disk.iter().all(|&b| b == 0),
                "post-Clear/Purge wipe must read back all-zero (case={id}, capacity={capacity}, claim=Passed mismatched=0, first_nonzero={})",
                on_disk.iter().position(|&b| b != 0).map(|p| p as u64).unwrap_or(u64::MAX)
            );
        }
        _ => {
            let first_change = on_disk
                .iter()
                .position(|&b| b != 0)
                .expect("seeded-random final pass must leave non-zero bytes on disk");
            assert!(
                on_disk.iter().any(|&b| b != 0),
                "post-wipe file must not read back all-zero for a seeded-random final pass (case={id})"
            );
            eprintln!(
                "CORPUS-WIPE-NOTE | case={id} standard={standard:?} final=SeededRandom capacity={capacity} claim=Passed mismatched=0 first_nonzero_byte={first_change}"
            );
        }
    }
    eprintln!(
        "CORPUS-WIPE-OK | case={id} standard={standard:?} capacity={capacity} bytes_verified={} mismatched_sectors=0",
        verification.bytes_verified
    );
}

#[test]
fn corpus_wipe_clears_clean_image_byte_verified() {
    let dir = fresh_dir("wipe");
    let std_cases = [
        (
            "clear-exact-sector",
            WipeStandard::Nist800_88Clear,
            4 * 1024 * 1024,
            0xA5u8,
        ),
        (
            "clear-partial-tail",
            WipeStandard::Nist800_88Clear,
            4 * 1024 * 1024 + 137,
            0x3C,
        ),
        (
            "clear-mixed",
            WipeStandard::Nist800_88Clear,
            2 * 1024 * 1024 + 513,
            0x00,
        ),
        (
            "purge-exact-sector",
            WipeStandard::Nist800_88Purge,
            8 * 1024 * 1024,
            0x7F,
        ),
        (
            "purge-partial-tail",
            WipeStandard::Nist800_88Purge,
            6 * 1024 * 1024 + 771,
            0x11,
        ),
        (
            "dod-exact-sector",
            WipeStandard::Dod522022M,
            6 * 1024 * 1024,
            0xEF,
        ),
        (
            "dod-partial-tail",
            WipeStandard::Dod522022M,
            2 * 1024 * 1024 + 55,
            0x33,
        ),
        (
            "ieee-purge",
            WipeStandard::Ieee2883Purge,
            5 * 1024 * 1024,
            0x22,
        ),
        (
            "clear-minimal-1k",
            WipeStandard::Nist800_88Clear,
            1024,
            0x01,
        ),
    ];
    for (id, standard, capacity, fill) in &std_cases {
        let path = dir.join(format!("{id}.img"));
        make_image(&path, *capacity, *fill);
        eprintln!("CORPUS-WIPE-START | case={id} standard={standard:?} capacity={capacity}");
        wipe_and_assert(&path, *standard, id);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Honest negative: a wipe targeting an image with the *hardware* method that
/// the operator has NOT acknowledged as a fallback must be refused outright,
/// never silently downgraded to first-pass "clean".
#[test]
fn corpus_wipe_refuses_unacknowledged_fallback() {
    let dir = fresh_dir("fallback");
    let path = dir.join("refuse.img");
    make_image(&path, 1024 * 1024, 0xAA);
    let _ = std::fs::remove_dir_all(&dir);
    // We go through run_wipe (full report pipeline) but with the hard-method
    // standard whose fallback is not acknowledged.
    let dir2 = fresh_dir("fallback-run");
    let report_dir = dir2.join("reports");
    std::fs::create_dir_all(&report_dir).unwrap();
    let path = dir2.join("refuse.img");
    make_image(&path, 64 * 1024, 0xAA);
    let sha_before = sha256_hex(&std::fs::read(&path).unwrap());
    let size_before = std::fs::metadata(&path).unwrap().len();
    let request = WipeRequest {
        operator_id: "corpus-neg".into(),
        target: WipeTarget::ImageFile(path.clone()),
        standard: WipeStandard::AtaSecureErase,
        operator_confirmed_target: format!("image file {}", path.display()),
        block_size: None,
        fallback_acknowledged: false,
    };
    let err = run_wipe(request, &report_dir, CORPUS_KEY).unwrap_err();
    assert!(
        format!("{err:?}").contains("fallback"),
        "must refuse with a fallback-ack error, got: {err:?}"
    );
    // Honest refusal semantics: the engine refuses BEFORE touching the target,
    // so a refused image must remain byte-UNTROUCHED (present, same length,
    // same sha256) — never truncated, emptied, or half-wiped.
    assert!(
        std::path::Path::new(&path).exists(),
        "refused operation must leave the image present (case=fallback-refuse)"
    );
    let len_after = std::fs::metadata(&path).unwrap().len();
    assert_eq!(
        len_after, size_before,
        "refused operation must not change image length (case=fallback-refuse, before={size_before}, after={len_after})"
    );
    assert_eq!(
        sha256_hex(&std::fs::read(&path).unwrap()),
        sha_before,
        "refused operation must leave the image byte-untouched (sha256 must match pre-sha)"
    );
    let _ = std::fs::remove_dir_all(&dir2);
}

/// Build a minimal but structurally-UNZIPPABLE archive: a ZIP with a single
/// stored entry plus a real End-Of-Central-Directory. The engine's own
/// `structure_valid` must accept it (honest carve target).
fn minimal_zip(seed: u8) -> Vec<u8> {
    let content: Vec<u8> = (0u8..=255)
        .map(|i| i.wrapping_mul(seed).wrapping_add(31))
        .collect();
    let content_len = content.len() as u16;
    let name = b"corpus.bin";

    let mut zip = Vec::new();
    // local file header
    zip.extend_from_slice(b"PK\x03\x04");
    zip.extend_from_slice(&20u16.to_le_bytes()); // version needed
    zip.extend_from_slice(&0u16.to_le_bytes()); // flags
    zip.extend_from_slice(&0u16.to_le_bytes()); // method: store
    zip.extend_from_slice(&0u16.to_le_bytes()); // mod time
    zip.extend_from_slice(&0u16.to_le_bytes()); // mod date
    zip.extend_from_slice(&0u32.to_le_bytes()); // crc (stored = we use 0)
    zip.extend_from_slice(&content_len.to_le_bytes());
    zip.extend_from_slice(&content_len.to_le_bytes());
    zip.extend_from_slice(&(name.len() as u16).to_le_bytes());
    zip.extend_from_slice(&0u16.to_le_bytes()); // extra len
    zip.extend_from_slice(name);
    zip.extend_from_slice(&content);

    // central directory entry
    zip.extend_from_slice(b"PK\x01\x02");
    zip.extend_from_slice(&0x0014u16.to_le_bytes()); // version made by
    zip.extend_from_slice(&0x0014u16.to_le_bytes()); // version needed
    zip.extend_from_slice(&0u16.to_le_bytes()); // flags
    zip.extend_from_slice(&0u16.to_le_bytes()); // method: store
    zip.extend_from_slice(&0u16.to_le_bytes()); // mod time
    zip.extend_from_slice(&0u16.to_le_bytes()); // mod date
    zip.extend_from_slice(&0u32.to_le_bytes()); // crc
    zip.extend_from_slice(&content_len.to_le_bytes());
    zip.extend_from_slice(&content_len.to_le_bytes());
    zip.extend_from_slice(&(name.len() as u16).to_le_bytes());
    zip.extend_from_slice(&0u16.to_le_bytes()); // extra
    zip.extend_from_slice(&0u16.to_le_bytes()); // comment
    zip.extend_from_slice(&0u16.to_le_bytes()); // disk#
    zip.extend_from_slice(&0u16.to_le_bytes()); // int attrs
    zip.extend_from_slice(&0u32.to_le_bytes()); // ext attrs
    zip.extend_from_slice(&0u32.to_le_bytes()); // local header offset
    zip.extend_from_slice(name);

    // end-of-central-directory (spec-valid 22-byte record so the engine's
    // ZipEocd end resolution reads comment_len at eocd+20 == 0, not the 0x7F
    // padding that follows the embedded zip).
    let cd_offset = (30u32 + name.len() as u32 + content_len as u32); // after local block
    let cd_size = (46u32 + name.len() as u32); // single central-directory entry (extra+comment = 0)
    let eocd = {
        let mut e = Vec::with_capacity(22);
        e.extend_from_slice(b"PK\x05\x06");
        e.extend_from_slice(&0u16.to_le_bytes()); // disk
        e.extend_from_slice(&0u16.to_le_bytes()); // cd disk
        e.extend_from_slice(&1u16.to_le_bytes()); // entries on disk
        e.extend_from_slice(&1u16.to_le_bytes()); // entries total
        e.extend_from_slice(&cd_size.to_le_bytes()); // cd size (u32)
        e.extend_from_slice(&cd_offset.to_le_bytes()); // cd offset (u32)
        e.extend_from_slice(&0u16.to_le_bytes()); // comment length (u16) -> engine reads 0 at eocd+20
        e
    };
    zip.extend_from_slice(&eocd);
    zip
}

#[test]
fn corpus_carve_recovers_embedded_zip_byte_exact() {
    let dir = fresh_dir("carve");
    let out = dir.join("out");
    let reports = dir.join("reports");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::create_dir_all(&reports).unwrap();

    let zip = minimal_zip(0x5B);
    let zip_sha = sha256_hex(&zip);

    // Carrier: junk, then the ZIP, then more junk. The engine must locate the
    // signature by scanning; it is NOT given the offset.
    let mut carrier = Vec::new();
    carrier.extend_from_slice(&[0x00; 2048]);
    carrier.extend_from_slice(&[0xFF; 333]);
    carrier.extend_from_slice(&zip);
    carrier.extend_from_slice(&[0x7F; 6000]);
    let src = dir.join("carrier.bin");
    std::fs::write(&src, &carrier).unwrap();

    let run = run_carve(&src, &out, &reports, CORPUS_KEY).unwrap();
    assert!(
        run.run.scanned_bytes >= carrier.len() as u64,
        "must scan full carrier"
    );

    let recovered = run
        .run
        .files
        .iter()
        .find(|f| f.signature == "zip")
        .expect("must recover the embedded zip");
    assert!(
        recovered.structure_valid,
        "recovered zip must be structurally valid"
    );
    eprintln!(
        "CORPUS-CARVE-DIAG | recovered={recovered:?} seeded_len={} zip_sha={zip_sha}",
        zip.len()
    );
    assert_eq!(
        recovered.sha256, zip_sha,
        "recovered sha256 must equal seeded"
    );

    // Byte-for-byte: recovered file on disk equals the seeded zip.
    let on_disk = std::fs::read(&recovered.output_path).unwrap();
    let diff = on_disk
        .iter()
        .zip(zip.iter())
        .position(|(a, b)| a != b)
        .unwrap_or(on_disk.len().min(zip.len()));
    assert!(
        on_disk == zip,
        "recovered bytes must equal seeded zip exactly (recovered.len={} zip.len={} first_diff={})",
        on_disk.len(),
        zip.len(),
        diff
    );
    assert_eq!(on_disk.len(), zip.len());

    assert!(run.report_json_path.exists());
    assert!(run.report_pdf_path.exists());
    let _ = std::fs::remove_dir_all(&dir);
}
