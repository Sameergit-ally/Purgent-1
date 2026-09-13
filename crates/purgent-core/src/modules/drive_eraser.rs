use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use uuid::Uuid;
use zeroize::Zeroize;

use super::config::{self, PassPattern, WipeSpec, WipeStandard};
use super::log;

const SECTOR_SIZE: u64 = 512;
const DEFAULT_BLOCK_SIZE: u64 = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WipeTarget {
    ImageFile(PathBuf),
    Device { path: String, capacity_bytes: u64 },
}

impl WipeTarget {
    pub fn display(&self) -> String {
        match self {
            WipeTarget::ImageFile(p) => format!("image file {}", p.display()),
            WipeTarget::Device {
                path,
                capacity_bytes,
            } => {
                format!("device {path} ({capacity_bytes} bytes)")
            }
        }
    }

    fn capacity_bytes(&self) -> Result<u64, WipeError> {
        match self {
            WipeTarget::ImageFile(p) => {
                let meta = std::fs::metadata(p).map_err(|e| WipeError::Target(e.to_string()))?;
                Ok(meta.len())
            }
            WipeTarget::Device { capacity_bytes, .. } => Ok(*capacity_bytes),
        }
    }

    fn open(&self) -> Result<File, WipeError> {
        match self {
            WipeTarget::ImageFile(p) => std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(p)
                .map_err(|e| WipeError::Target(e.to_string())),
            WipeTarget::Device { path, .. } => {
                #[cfg(windows)]
                {
                    open_device_windows(path)
                }
                #[cfg(target_os = "linux")]
                {
                    open_device_linux(path)
                }
                #[cfg(not(any(windows, target_os = "linux")))]
                {
                    let _ = path;
                    Err(WipeError::Target(
                        "device wiping is not implemented on this platform build".into(),
                    ))
                }
            }
        }
    }
}

#[cfg(windows)]
fn open_device_windows(path: &str) -> Result<File, WipeError> {
    use std::os::windows::io::FromRawHandle;
    use windows_sys::Win32::Foundation::{GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };

    let wide: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            GENERIC_WRITE | GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(WipeError::Target(format!(
            "cannot open device {path} for read/write (admin required)"
        )));
    }
    Ok(unsafe { File::from_raw_handle(handle as *mut _) })
}

#[cfg(target_os = "linux")]
fn open_device_linux(path: &str) -> Result<File, WipeError> {
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|e| {
            WipeError::Target(format!(
                "cannot open device {path} read/write (root required): {e}"
            ))
        })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WipeError {
    ConfirmationMismatch,
    Target(String),
    Reporting(String),
    Io(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WipePhase {
    Running,
    Verifying,
    Done,
}

#[derive(Debug, Clone, Copy)]
pub struct WipeProgress {
    pub phase: WipePhase,
    pub pass: usize,
    pub total_passes: usize,
    pub bytes_done: u64,
    pub total_bytes: u64,
}

pub struct WipeRequest {
    pub operator_id: String,
    pub target: WipeTarget,
    pub standard: WipeStandard,
    pub operator_confirmed_target: String,
    pub block_size: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VerificationResult {
    pub status: VerificationStatus,
    pub bytes_verified: u64,
    pub mismatched_sectors: u64,
    pub detail: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct WipeResult {
    pub operation_id: String,
    pub target: String,
    pub standard: WipeStandard,
    pub standard_label: String,
    pub capacity_bytes: u64,
    pub started_at: String,
    pub finished_at: String,
    pub operator_id: String,
    pub verification: Option<VerificationResult>,
    pub skipped_sectors: Vec<u64>,
    pub complete: bool,
}

impl WipeResult {
    pub fn is_complete(&self) -> bool {
        self.complete
    }
}

pub fn wipe(request: WipeRequest) -> Result<WipeResult, WipeError> {
    wipe_with_progress(request, &mut |_| {})
}

pub fn wipe_with_progress(
    request: WipeRequest,
    progress: super::progress::ProgressFn,
) -> Result<WipeResult, WipeError> {
    if request.operator_confirmed_target != request.target.display() {
        log::log(
            "selected",
            &format!(
                "wipe REFUSED: operator confirmation '{}' does not match target '{}'",
                request.operator_confirmed_target,
                request.target.display()
            ),
        );
        return Err(WipeError::ConfirmationMismatch);
    }

    let operation_id = Uuid::new_v4().to_string();
    let started_at = log::now_utc_rfc3339();
    let spec = config::wipe_spec(request.standard);
    let capacity = request.target.capacity_bytes()?;
    let block_size = request
        .block_size
        .unwrap_or(DEFAULT_BLOCK_SIZE)
        .max(SECTOR_SIZE);

    log::log(
        "selected",
        &format!(
            "operation={operation_id} operator={} target={} standard={} capacity={capacity}",
            request.operator_id,
            request.target.display(),
            spec.standard.label()
        ),
    );

    if capacity == 0 || capacity < SECTOR_SIZE {
        return Err(WipeError::Target("target has no sectors to wipe".into()));
    }

    let mut file = request.target.open()?;
    wipe_with_spec(
        &mut file,
        capacity,
        &spec,
        request.standard,
        block_size,
        &operation_id,
        &request.operator_id,
        request.target.display(),
        started_at,
        progress,
    )
}

fn wipe_with_spec(
    file: &mut File,
    capacity: u64,
    spec: &WipeSpec,
    standard: WipeStandard,
    block_size: u64,
    operation_id: &str,
    operator_id: &str,
    target_desc: String,
    started_at: String,
    progress: super::progress::ProgressFn,
) -> Result<WipeResult, WipeError> {
    let mut skipped_sectors: Vec<u64> = Vec::new();
    let total_passes = spec.passes.len();

    for (pass_index, pattern) in spec.passes.iter().enumerate() {
        log::log(
            "running",
            &format!(
                "operation={operation_id} pass={}/{} pattern={:?}",
                pass_index + 1,
                total_passes,
                pattern
            ),
        );
        write_pass(
            file,
            capacity,
            *pattern,
            spec.seed,
            pass_index,
            block_size,
            &mut skipped_sectors,
            operation_id,
            progress,
        )?;
    }

    log::log(
        "verifying",
        &format!("operation={operation_id} read-back of {capacity} bytes"),
    );
    let verification = verify_pass(
        file,
        capacity,
        spec,
        block_size,
        &mut skipped_sectors,
        operation_id,
        progress,
    )?;

    let finished_at = log::now_utc_rfc3339();
    let complete = verification.status == VerificationStatus::Passed;
    log::log(
        if complete { "verified" } else { "failed" },
        &format!(
            "operation={operation_id} status={:?} mismatched_sectors={}",
            verification.status, verification.mismatched_sectors
        ),
    );

    let result = WipeResult {
        operation_id: operation_id.to_string(),
        target: target_desc,
        standard,
        standard_label: standard.label().to_string(),
        capacity_bytes: capacity,
        started_at,
        finished_at,
        operator_id: operator_id.to_string(),
        verification: Some(verification),
        skipped_sectors,
        complete,
    };

    log::log(
        "reported",
        &format!(
            "operation={operation_id} report_ready=true complete={}",
            result.complete
        ),
    );
    Ok(result)
}

fn write_pass(
    file: &mut File,
    capacity: u64,
    pattern: PassPattern,
    seed: u64,
    pass_index: usize,
    block_size: u64,
    skipped_sectors: &mut Vec<u64>,
    operation_id: &str,
    progress: super::progress::ProgressFn,
) -> Result<(), WipeError> {
    file.seek(SeekFrom::Start(0))
        .map_err(|e| WipeError::Io(e.to_string()))?;

    let mut block = vec![0u8; block_size as usize];
    fill_pattern(&mut block, pattern, seed, pass_index);

    let mut offset = 0u64;
    while offset < capacity {
        let remaining = capacity - offset;
        let chunk = if remaining < block_size {
            remaining as usize
        } else {
            block_size as usize
        };
        if chunk < block.len() {
            block.truncate(chunk);
        } else if block.len() < chunk {
            block.resize(chunk, 0);
            fill_pattern(&mut block, pattern, seed, pass_index);
        }
        if let Err(e) = file
            .seek(SeekFrom::Start(offset))
            .and_then(|_| file.write_all(&block[..chunk]))
        {
            let sector = offset / SECTOR_SIZE;
            skipped_sectors.push(sector);
            log::log(
                "running",
                &format!("bad sector {sector} at offset {offset} skipped: {e}"),
            );
        }
        offset += chunk as u64;
        progress(super::progress::ProgressUpdate {
            operation_id: operation_id.to_string(),
            operation_type: "secure_erase".to_string(),
            phase: "pass".to_string(),
            bytes_done: offset,
            total_bytes: capacity,
            message: format!("pass {} writing", pass_index + 1),
        });
    }
    block.zeroize();
    file.flush().map_err(|e| WipeError::Io(e.to_string()))?;
    Ok(())
}

fn verify_pass(
    file: &mut File,
    capacity: u64,
    spec: &WipeSpec,
    block_size: u64,
    skipped_sectors: &mut Vec<u64>,
    operation_id: &str,
    progress: super::progress::ProgressFn,
) -> Result<VerificationResult, WipeError> {
    file.seek(SeekFrom::Start(0))
        .map_err(|e| WipeError::Io(e.to_string()))?;

    let final_pass_index = spec.passes.len() - 1;
    let expected = spec.passes[final_pass_index];

    let mut expected_block = vec![0u8; block_size as usize];
    fill_pattern(&mut expected_block, expected, spec.seed, final_pass_index);

    let mut read_buf = vec![0u8; block_size as usize];
    let mut bytes_verified = 0u64;
    let mut mismatched_sectors = 0u64;

    let mut offset = 0u64;
    while offset < capacity {
        let remaining = capacity - offset;
        let chunk = if remaining < block_size {
            remaining as usize
        } else {
            block_size as usize
        };
        if chunk > expected_block.len() {
            expected_block.resize(chunk, 0);
            fill_pattern(&mut expected_block, expected, spec.seed, final_pass_index);
        }
        let mut n = 0usize;
        match file
            .seek(SeekFrom::Start(offset))
            .and_then(|_| file.read(&mut read_buf[..chunk]))
        {
            Ok(read) => n = read,
            Err(e) => {
                let sector = offset / SECTOR_SIZE;
                skipped_sectors.push(sector);
                log::log(
                    "verifying",
                    &format!("unreadable sector {sector} at offset {offset} logged: {e}"),
                );
            }
        }
        if n > 0 {
            bytes_verified += n as u64;
            let sector_count = n as u64 / SECTOR_SIZE;
            for s in 0..sector_count {
                let start = (s * SECTOR_SIZE) as usize;
                let end = start + SECTOR_SIZE as usize;
                if read_buf[start..end] != expected_block[start..end] {
                    mismatched_sectors += 1;
                }
            }
        }
        offset += chunk as u64;
        progress(super::progress::ProgressUpdate {
            operation_id: operation_id.to_string(),
            operation_type: "secure_erase".to_string(),
            phase: "verify".to_string(),
            bytes_done: offset,
            total_bytes: capacity,
            message: "verifying read-back".to_string(),
        });
    }
    expected_block.zeroize();
    read_buf.zeroize();

    let (status, detail) = if mismatched_sectors == 0 {
        (
            VerificationStatus::Passed,
            format!(
                "read-back verified {bytes_verified} bytes ({} sectors)",
                bytes_verified / SECTOR_SIZE
            ),
        )
    } else {
        (VerificationStatus::Failed, format!("read-back mismatch on {mismatched_sectors} sectors after {bytes_verified} bytes verified"))
    };

    Ok(VerificationResult {
        status,
        bytes_verified,
        mismatched_sectors,
        detail,
    })
}

pub(crate) fn fill_pattern(buf: &mut [u8], pattern: PassPattern, seed: u64, pass_index: usize) {
    match pattern {
        PassPattern::Zeros => buf.fill(0x00),
        PassPattern::Ones => buf.fill(0xFF),
        PassPattern::Fixed(v) => buf.fill(v),
        PassPattern::SeededRandom => {
            let pass_seed = seed.wrapping_add(pass_index as u64);
            let mut rng = XorShift64::new(pass_seed);
            for chunk in buf.chunks_mut(8) {
                let bytes = rng.next().to_le_bytes();
                chunk.copy_from_slice(&bytes[..chunk.len()]);
            }
        }
    }
}

struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    fn new(seed: u64) -> Self {
        XorShift64 {
            state: if seed == 0 {
                0x9E37_79B9_7F4A_7C15
            } else {
                seed
            },
        }
    }

    fn next(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::config::WipeStandard;

    fn make_image(path: &std::path::Path, size: u64, fill: u8) {
        let mut f = File::create(path).unwrap();
        let block = vec![fill; 4096];
        let mut written = 0u64;
        while written < size {
            let n = std::cmp::min(size - written, block.len() as u64) as usize;
            f.write_all(&block[..n]).unwrap();
            written += n as u64;
        }
    }

    fn wipe_and_assert(path: &std::path::Path, standard: WipeStandard) -> WipeResult {
        let size = std::fs::metadata(path).unwrap().len();
        let result = wipe(WipeRequest {
            operator_id: "test-operator".into(),
            target: WipeTarget::ImageFile(path.to_path_buf()),
            standard,
            operator_confirmed_target: format!("image file {}", path.display()),
            block_size: None,
        })
        .expect("wipe succeeds");

        assert!(result.complete, "wipe must complete for {standard:?}");
        let verification = result.verification.as_ref().unwrap();
        assert_eq!(verification.status, VerificationStatus::Passed);
        assert_eq!(verification.bytes_verified, size);
        assert_eq!(verification.mismatched_sectors, 0);
        assert!(result.skipped_sectors.is_empty());
        assert!(!result.operation_id.is_empty());
        assert_eq!(result.standard, standard);
        assert_eq!(result.capacity_bytes, size);
        assert!(result.started_at < result.finished_at);
        result
    }

    #[test]
    fn nist_clear_wipes_image_and_verifies() {
        let dir = std::env::temp_dir().join(format!("purgent-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("disk.img");
        make_image(&path, 4 * 1024 * 1024 + 137, 0xA5);
        wipe_and_assert(&path, WipeStandard::Nist800_88Clear);
        let bytes = std::fs::read(&path).unwrap();
        assert!(
            bytes.iter().all(|&b| b == 0x00),
            "image must read back as zeros after NIST Clear"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn nist_purge_wipes_image_and_verifies() {
        let dir = std::env::temp_dir().join(format!("purgent-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("disk.img");
        make_image(&path, 2 * 1024 * 1024, 0x5A);
        wipe_and_assert(&path, WipeStandard::Nist800_88Purge);
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.iter().all(|&b| b == 0x00));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dod_3_pass_wipes_image_and_verifies() {
        let dir = std::env::temp_dir().join(format!("purgent-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("disk.img");
        make_image(&path, 3 * 1024 * 1024, 0x7E);
        wipe_and_assert(&path, WipeStandard::Dod522022M);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn verification_detects_leftover_data() {
        let dir = std::env::temp_dir().join(format!("purgent-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("dirty.img");
        make_image(&path, 1024 * 1024, 0x00);
        let result = wipe(WipeRequest {
            operator_id: "test-operator".into(),
            target: WipeTarget::ImageFile(path.clone()),
            standard: WipeStandard::Nist800_88Clear,
            operator_confirmed_target: format!("image file {}", path.display()),
            block_size: None,
        })
        .unwrap();
        assert!(result.complete, "clean wipe must complete");

        {
            let mut f = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
            f.seek(SeekFrom::Start(SECTOR_SIZE)).unwrap();
            f.write_all(&[0xAB; 512]).unwrap();
        }

        let mut f = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        let spec = crate::modules::config::wipe_spec(WipeStandard::Nist800_88Clear);
        let mut skipped = Vec::new();
        let v = verify_pass(
            &mut f,
            std::fs::metadata(&path).unwrap().len(),
            &spec,
            4096,
            &mut skipped,
            "test-op",
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(
            v.status,
            VerificationStatus::Failed,
            "corrupted sector must be detected"
        );
        assert!(v.mismatched_sectors >= 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn confirmation_mismatch_is_refused() {
        let dir = std::env::temp_dir().join(format!("purgent-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("disk.img");
        make_image(&path, 1024 * 1024, 0x11);
        let err = wipe(WipeRequest {
            operator_id: "test-operator".into(),
            target: WipeTarget::ImageFile(path.clone()),
            standard: WipeStandard::Nist800_88Clear,
            operator_confirmed_target: "device \\\\.\\PHYSICALDRIVE0 (999 bytes)".into(),
            block_size: None,
        })
        .unwrap_err();
        assert_eq!(err, WipeError::ConfirmationMismatch);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
