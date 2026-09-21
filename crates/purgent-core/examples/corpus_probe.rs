// Honest corpus probe: independent observer. No engine claims trusted.
use purgent_core::modules::config::WipeStandard;
use purgent_core::modules::drive_eraser::{
    wipe_with_progress, VerificationStatus, WipeRequest, WipeResult, WipeTarget,
};
use purgent_core::modules::hashing::sha256_hex;
use purgent_core::modules::operations::{run_carve, run_wipe};
use std::path::PathBuf;
use uuid::Uuid;
fn fresh() -> PathBuf {
    let d = std::env::temp_dir().join(format!("purgent-probe-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&d).unwrap();
    d
}
fn main() {
    // --- A) WIPE byte-truth: engine claim vs INDEPENDENT read (partial tail 6MiB+771, non-zero fill) ---
    let dir = fresh();
    let path = dir.join("tail.img");
    let capacity = 6u64 * 1024 * 1024 + 771;
    let fill: u8 = 0x11;
    {
        let mut f = std::fs::File::create(&path).unwrap();
        let blk = vec![fill; 4096];
        let mut written = 0u64;
        while written < capacity {
            let n = std::cmp::min(capacity - written, blk.len() as u64) as usize;
            f.write_all(&blk[..n]).unwrap();
            written += n as u64;
        }
    }
    let request = WipeRequest {
        operator_id: "probe-opt".into(),
        target: WipeTarget::ImageFile(path.clone()),
        standard: WipeStandard::Nist800_88Purge,
        operator_confirmed_target: format!("image file {}", path.display()),
        block_size: None,
        fallback_acknowledged: false,
    };
    let result = wipe_with_progress(request, &mut |_| {}).unwrap();
    let v = result.verification.as_ref().unwrap();
    println!("PROBE-WIPE-ENGINE | status={:?} bytes_verified={} mismatched_sectors={} mismatched_bytes={}", v.status, v.bytes_verified, v.mismatched_sectors, v.mismatched_bytes);
    // INDEPENDENT: fresh File::open + read, count non-zero bytes + first offset
    let on_disk = std::fs::read(&path).unwrap();
    let nz: Vec<(usize, u8)> = on_disk
        .iter()
        .enumerate()
        .filter(|(_, &b)| b != 0)
        .take(6)
        .collect();
    let nz_count = on_disk.iter().filter(|&&b| b != 0).count();
    println!(
        "PROBE-WIPE-OWN | len={} nonzero_count={} first_nonzero={:?} capacity={capacity}",
        on_disk.len(),
        nz_count,
        nz
    );

    // --- B) CARVE byte-truth: seeded zip sha vs engine recovered sha + recovered content len ---
    let zip = {
        let content: Vec<u8> = (0u8..=255)
            .map(|i| i.wrapping_mul(0x5B).wrapping_add(31))
            .collect();
        let content_len = content.len() as u16;
        let name = b"corpus.bin";
        let mut zip = Vec::new();
        zip.extend_from_slice(b"PK\x03\x04");
        zip.extend_from_slice(&20u16.to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());
        zip.extend_from_slice(&0u32.to_le_bytes());
        zip.extend_from_slice(&content_len.to_le_bytes());
        zip.extend_from_slice(&content_len.to_le_bytes());
        zip.extend_from_slice(&(name.len() as u16).to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());
        zip.extend_from_slice(name);
        zip.extend_from_slice(&content);
        zip.extend_from_slice(b"PK\x01\x02");
        zip.extend_from_slice(&0x0014u16.to_le_bytes());
        zip.extend_from_slice(&0x0014u16.to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());
        zip.extend_from_slice(&0u32.to_le_bytes());
        zip.extend_from_slice(&content_len.to_le_bytes());
        zip.extend_from_slice(&content_len.to_le_bytes());
        zip.extend_from_slice(&(name.len() as u16).to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());
        zip.extend_from_slice(&0u32.to_le_bytes());
        zip.extend_from_slice(&0u32.to_le_bytes());
        zip.extend_from_slice(name);
        zip.extend_from_slice(b"PK\x05\x06");
        zip.extend_from_slice(&0u16.to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());
        zip.extend_from_slice(&1u16.to_le_bytes());
        zip.extend_from_slice(&1u16.to_le_bytes());
        zip.extend_from_slice(&(name.len() as u16).to_le_bytes());
        zip.extend_from_slice(&0u32.to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());
        zip
    };
    let zip_sha = sha256_hex(&zip);
    let zip_len = zip.len() as u64;
    let mut carrier = Vec::new();
    carrier.extend_from_slice(&[0x00; 2048]);
    carrier.extend_from_slice(&[0xFF; 333]);
    carrier.extend_from_slice(&zip);
    carrier.extend_from_slice(&[0x7F; 6000]);
    let src = dir.join("carrier.bin");
    std::fs::write(&src, &carrier).unwrap();
    let out = dir.join("out");
    std::fs::create_dir_all(&out).unwrap();
    let reports = dir.join("reports");
    std::fs::create_dir_all(&reports).unwrap();
    let carve = run_carve(&src, &out, &reports, b"probe-key-00000000000000").unwrap();
    let run = carve.run;
    let rec = run.files.iter().find(|f| f.signature == "zip").unwrap();
    let on_disk = std::fs::read(&rec.output_path).unwrap();
    let on_disk_sha = sha256_hex(&on_disk);
    println!(
        "PROBE-CARVE-OWN | seeded_len={} seeded_sha={}",
        zip_len, zip_sha
    );
    println!("PROBE-CARVE-ENGINE | recovered_signature={} recovered_structure_valid={} recovered_sha={} recovered_meta_bytes={} output_path={}", rec.signature, rec.structure_valid, rec.sha256, rec.size_bytes, rec.output_path);
    println!(
        "PROBE-CARVE-OWN   | on_disk_len={} on_disk_sha={}",
        on_disk.len(),
        on_disk_sha
    );
    println!("CARVE-SHA-EQUAL(seeded==engine)={} CARVE-SHA-EQUAL(seeded==ondisk)={} CARVE-BYTES-EQUAL={}", zip_sha==rec.sha256, zip_sha==on_disk_sha, zip==on_disk);
    let _ = std::fs::remove_dir_all(&dir);
}
