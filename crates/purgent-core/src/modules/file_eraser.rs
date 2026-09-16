use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use uuid::Uuid;
use zeroize::Zeroize;

use super::config::{PassPattern, RANDOM_SEED};
use super::drive_eraser::fill_pattern;
use super::log;

const SECTOR_SIZE: u64 = 512;
const DEFAULT_BLOCK_SIZE: u64 = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileEraseStandard {
    NistClear,
    Purge3Pass,
}

impl FileEraseStandard {
    pub fn label(self) -> &'static str {
        match self {
            FileEraseStandard::NistClear => "NIST SP 800-88 Rev.1 Clear (1 pass)",
            FileEraseStandard::Purge3Pass => "Purge (3 pass): zeros, 0xFF, random",
        }
    }

    fn passes(self) -> Vec<PassPattern> {
        match self {
            FileEraseStandard::NistClear => vec![PassPattern::Zeros],
            FileEraseStandard::Purge3Pass => {
                vec![
                    PassPattern::Zeros,
                    PassPattern::Ones,
                    PassPattern::SeededRandom,
                ]
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EraseTarget {
    SingleFile(PathBuf),
    Folder(PathBuf),
}

impl EraseTarget {
    pub fn display(&self) -> String {
        match self {
            EraseTarget::SingleFile(p) => format!("file {}", p.display()),
            EraseTarget::Folder(p) => format!("folder {}", p.display()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EraseError {
    ConfirmationMismatch,
    NotFound(String),
    ExclusivityDenied(String),
    Irrecoverable(String),
    Io(String),
}

pub struct EraseRequest {
    pub operator_id: String,
    pub target: EraseTarget,
    pub standard: FileEraseStandard,
    pub operator_confirmed_target: String,
    pub block_size: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EraseVerificationStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EraseFileRecord {
    pub path: String,
    pub bytes_erased: u64,
    pub bytes_verified: u64,
    pub mismatched_sectors: u64,
    pub skipped_sectors: u64,
    pub verified: bool,
    pub deleted: bool,
    pub trace_scrub: Option<super::trace_scrubber::TraceScrubRecord>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileEraseResult {
    pub operation_id: String,
    pub target: String,
    pub standard: FileEraseStandard,
    pub standard_label: String,
    pub started_at: String,
    pub finished_at: String,
    pub operator_id: String,
    pub files: Vec<EraseFileRecord>,
    pub directories_removed: u64,
    pub complete: bool,
    pub skipped_sectors: u64,
    pub trace_scrub: Option<super::trace_scrubber::TraceScrubRecord>,
}

impl FileEraseResult {
    pub fn is_complete(&self) -> bool {
        self.complete
    }

    pub fn all_verified_and_deleted(&self) -> bool {
        self.files.iter().all(|f| f.verified && f.deleted)
    }
}

pub fn erase(request: EraseRequest) -> Result<FileEraseResult, EraseError> {
    erase_with_progress(request, &mut |_| {})
}

pub fn erase_with_progress(
    request: EraseRequest,
    progress: super::progress::ProgressFn,
) -> Result<FileEraseResult, EraseError> {
    if request.operator_confirmed_target != request.target.display() {
        log::log(
            "selected",
            &format!(
                "erase REFUSED: operator confirmation '{}' does not match target '{}'",
                request.operator_confirmed_target,
                request.target.display()
            ),
        );
        return Err(EraseError::ConfirmationMismatch);
    }

    let operation_id = Uuid::new_v4().to_string();
    let started_at = log::now_utc_rfc3339();
    let block_size = request
        .block_size
        .unwrap_or(DEFAULT_BLOCK_SIZE)
        .max(SECTOR_SIZE);
    let passes = request.standard.passes();

    log::log(
        "selected",
        &format!(
            "operation={operation_id} operator={} target={} standard={}",
            request.operator_id,
            request.target.display(),
            request.standard.label()
        ),
    );

    let mut result = FileEraseResult {
        operation_id: operation_id.clone(),
        target: request.target.display(),
        standard: request.standard,
        standard_label: request.standard.label().to_string(),
        started_at,
        finished_at: String::new(),
        operator_id: request.operator_id.clone(),
        files: Vec::new(),
        directories_removed: 0,
        skipped_sectors: 0,
        complete: false,
        trace_scrub: None,
    };

    let job_files: Vec<PathBuf> = match &request.target {
        EraseTarget::SingleFile(path) => {
            verify_or_refuse_path(path)?;
            vec![path.clone()]
        }
        EraseTarget::Folder(path) => {
            verify_or_refuse_path(path)?;
            let mut files = Vec::new();
            collect_files(path, &mut files)?;
            if files.is_empty() {
                return Err(EraseError::NotFound(format!(
                    "folder {} contains no files",
                    path.display()
                )));
            }
            files
        }
    };
    let total_bytes: u64 = job_files
        .iter()
        .map(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0))
        .sum();

    let mut base_done = 0u64;
    for file in &job_files {
        let (record, _) = erase_one_file(
            file,
            block_size,
            &passes,
            &operation_id,
            base_done,
            total_bytes,
            progress,
        )?;
        base_done += record.bytes_erased;
        result.files.push(record);
    }

    if let EraseTarget::Folder(path) = &request.target {
        result.directories_removed = remove_empty_dirs_bottom_up(path)?;
    }

    let finished_at = log::now_utc_rfc3339();
    result.finished_at = finished_at.clone();
    result.complete = result.all_verified_and_deleted();
    result.trace_scrub = Some(super::trace_scrubber::scrub_trace(&match &request.target {
        EraseTarget::SingleFile(path) => path.clone(),
        EraseTarget::Folder(path) => path.clone(),
    }));
    log::log(
        if result.complete {
            "verified"
        } else {
            "failed"
        },
        &format!(
            "operation={} files={} complete={} trace_scrub_ok={} trace_scrub_best_effort={}",
            result.operation_id,
            result.files.len(),
            result.complete,
            result
                .trace_scrub
                .as_ref()
                .map(|t| t.ok_count())
                .unwrap_or(0),
            result
                .trace_scrub
                .as_ref()
                .map(|t| t.best_effort_count())
                .unwrap_or(0)
        ),
    );
    log::log(
        "reported",
        &format!("operation={} report_ready=true", result.operation_id),
    );
    progress(super::progress::ProgressUpdate {
        operation_id,
        operation_type: "file_erase".to_string(),
        phase: "done".to_string(),
        bytes_done: total_bytes,
        total_bytes,
        message: "erase complete".to_string(),
    });
    Ok(result)
}

fn verify_or_refuse_path(path: &Path) -> Result<(), EraseError> {
    let meta = std::fs::metadata(path)
        .map_err(|e| EraseError::NotFound(format!("{}: {e}", path.display())))?;
    if !meta.is_file() && !meta.is_dir() {
        return Err(EraseError::NotFound(format!(
            "{} is neither file nor folder",
            path.display()
        )));
    }
    Ok(())
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), EraseError> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| EraseError::Io(format!("read_dir {}: {e}", dir.display())))?;
    for entry in entries {
        let entry = entry.map_err(|e| EraseError::Io(e.to_string()))?;
        let path = entry.path();
        if entry.file_type().map(|t| t.is_symlink()).unwrap_or(false) {
            log::log(
                "skipped",
                &format!(
                    "symlink={} NOT collected; wipe of folder refuses to follow links",
                    path.display()
                ),
            );
            continue;
        }
        let meta = entry
            .metadata()
            .map_err(|e| EraseError::Io(format!("metadata {}: {e}", path.display())))?;
        if meta.is_dir() {
            collect_files(&path, out)?;
        } else if meta.is_file() {
            out.push(path);
        }
    }
    Ok(())
}

fn remove_empty_dirs_bottom_up(dir: &Path) -> Result<u64, EraseError> {
    let entries = std::fs::read_dir(dir).map_err(|e| EraseError::Io(e.to_string()))?;
    let mut removed = 0u64;
    for entry in entries {
        let entry = entry.map_err(|e| EraseError::Io(e.to_string()))?;
        let path = entry.path();
        let meta = entry
            .metadata()
            .map_err(|e| EraseError::Io(e.to_string()))?;
        if meta.is_dir() {
            removed += remove_empty_dirs_bottom_up(&path)?;
        }
    }
    if std::fs::remove_dir(dir).is_ok() {
        removed += 1;
    }
    Ok(removed)
}

fn open_exclusive(path: &Path) -> Result<File, EraseError> {
    let mut opts = std::fs::OpenOptions::new();
    opts.read(true).write(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        opts.share_mode(0);
    }
    opts.open(path).map_err(|_| {
        EraseError::ExclusivityDenied("file is open by another process; wipe refused".to_string())
    })
}

#[allow(clippy::too_many_arguments)]
fn erase_one_file(
    path: &Path,
    block_size: u64,
    passes: &[PassPattern],
    operation_id: &str,
    base_done: u64,
    total_bytes: u64,
    progress: super::progress::ProgressFn,
) -> Result<(EraseFileRecord, Vec<u64>), EraseError> {
    let mut file = open_exclusive(path)?;
    let capacity = file
        .metadata()
        .map_err(|e| EraseError::Io(e.to_string()))?
        .len();

    let mut skipped_sectors: Vec<u64> = Vec::new();
    for (pass_index, pattern) in passes.iter().enumerate() {
        log::log(
            "running",
            &format!(
                "file={} pass={}/{} pattern={:?}",
                path.display(),
                pass_index + 1,
                passes.len(),
                pattern
            ),
        );
        write_pass(
            &mut file,
            capacity,
            *pattern,
            pass_index,
            block_size,
            &mut skipped_sectors,
            operation_id,
            path,
            base_done,
            total_bytes,
            progress,
        )?;
    }

    log::log(
        "verifying",
        &format!("file={} read-back of {capacity} bytes", path.display()),
    );
    let (mismatched, bytes_verified) = verify_file(
        &mut file,
        capacity,
        passes,
        block_size,
        &mut skipped_sectors,
        operation_id,
        path,
        base_done,
        total_bytes,
        progress,
    )?;

    let verified = mismatched == 0;
    if !verified {
        log::log(
            "failed",
            &format!("file={} mismatched_sectors={}", path.display(), mismatched),
        );
        log::log(
            "verified",
            &format!("file={} bytes_verified={bytes_verified}", path.display()),
        );
        return Ok((
            EraseFileRecord {
                path: path.display().to_string(),
                bytes_erased: capacity,
                bytes_verified: 0,
                mismatched_sectors: mismatched,
                skipped_sectors: 0,
                verified: false,
                deleted: false,
                trace_scrub: None,
            },
            skipped_sectors,
        ));
    }

    let fully_verified =
        mismatched == 0 && bytes_verified == capacity && skipped_sectors.is_empty();
    drop(file);
    let deleted = if fully_verified {
        let deleted = std::fs::remove_file(path).is_ok();
        if deleted {
            log::log(
                "verified",
                &format!("file={} deleted_ok=true", path.display()),
            );
        } else {
            log::log(
                "failed",
                &format!("file={} deletion failed", path.display()),
            );
        }
        deleted
    } else {
        log::log(
            "failed",
            &format!(
                "file={} NOT deleted because verification was not fully clean \
                 (mismatched={} verified={bytes_verified}/capacity={capacity} skipped={})",
                path.display(),
                mismatched,
                skipped_sectors.len()
            ),
        );
        false
    };

    Ok((
        EraseFileRecord {
            path: path.display().to_string(),
            bytes_erased: capacity,
            bytes_verified,
            mismatched_sectors: mismatched,
            skipped_sectors: skipped_sectors.len() as u64,
            verified: fully_verified,
            deleted,
            trace_scrub: None,
        },
        skipped_sectors,
    ))
}

#[allow(clippy::too_many_arguments)]
fn write_pass(
    file: &mut File,
    capacity: u64,
    pattern: PassPattern,
    pass_index: usize,
    block_size: u64,
    skipped_sectors: &mut Vec<u64>,
    operation_id: &str,
    path: &Path,
    base_done: u64,
    total_bytes: u64,
    progress: super::progress::ProgressFn,
) -> Result<(), EraseError> {
    file.seek(SeekFrom::Start(0))
        .map_err(|e| EraseError::Io(e.to_string()))?;

    let mut block = vec![0u8; block_size as usize];
    fill_pattern(&mut block, pattern, RANDOM_SEED, pass_index);

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
            fill_pattern(&mut block, pattern, RANDOM_SEED, pass_index);
        }
        if let Err(e) = file
            .seek(SeekFrom::Start(offset))
            .and_then(|_| file.write_all(&block[..chunk]))
        {
            skipped_sectors.push(offset / SECTOR_SIZE);
            log::log(
                "running",
                &format!(
                    "sector {} at offset {offset} write failed and is skipped: {e}",
                    offset / SECTOR_SIZE
                ),
            );
        }
        offset += chunk as u64;
        progress(super::progress::ProgressUpdate {
            operation_id: operation_id.to_string(),
            operation_type: "file_erase".to_string(),
            phase: "pass".to_string(),
            bytes_done: (base_done + offset).min(total_bytes),
            total_bytes,
            message: format!("erasing {}", path.display()),
        });
    }
    block.zeroize();
    file.flush().map_err(|e| EraseError::Io(e.to_string()))?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn verify_file(
    file: &mut File,
    capacity: u64,
    passes: &[PassPattern],
    block_size: u64,
    skipped_sectors: &mut Vec<u64>,
    operation_id: &str,
    path: &Path,
    base_done: u64,
    total_bytes: u64,
    progress: super::progress::ProgressFn,
) -> Result<(u64, u64), EraseError> {
    file.seek(SeekFrom::Start(0))
        .map_err(|e| EraseError::Io(e.to_string()))?;

    let final_pass_index = passes.len() - 1;
    let expected = passes[final_pass_index];

    let mut expected_block = vec![0u8; block_size as usize];
    fill_pattern(&mut expected_block, expected, RANDOM_SEED, final_pass_index);

    let mut read_buf = vec![0u8; block_size as usize];
    let mut mismatched = 0u64;
    let mut bytes_verified = 0u64;

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
            fill_pattern(&mut expected_block, expected, RANDOM_SEED, final_pass_index);
        }
        let mut n = 0usize;
        match file
            .seek(SeekFrom::Start(offset))
            .and_then(|_| file.read(&mut read_buf[..chunk]))
        {
            Ok(read) => n = read,
            Err(e) => {
                skipped_sectors.push(offset / SECTOR_SIZE);
                log::log(
                    "verifying",
                    &format!(
                        "sector {} at offset {offset} unreadable, logged: {e}",
                        offset / SECTOR_SIZE
                    ),
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
                    mismatched += 1;
                }
            }
        }
        offset += chunk as u64;
        progress(super::progress::ProgressUpdate {
            operation_id: operation_id.to_string(),
            operation_type: "file_erase".to_string(),
            phase: "verify".to_string(),
            bytes_done: (base_done + offset).min(total_bytes),
            total_bytes,
            message: format!("verifying {}", path.display()),
        });
    }
    expected_block.zeroize();
    read_buf.zeroize();
    Ok((mismatched, bytes_verified))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::log;

    fn write_file(path: &Path, size: usize, fill: u8) {
        let mut f = File::create(path).unwrap();
        let block = vec![fill; 4096];
        let mut written = 0usize;
        while written < size {
            let n = (size - written).min(block.len());
            f.write_all(&block[..n]).unwrap();
            written += n;
        }
    }

    fn setup_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("purgent-filetest-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn nist_clear_request(path: PathBuf) -> EraseRequest {
        let target = EraseTarget::SingleFile(path.clone());
        EraseRequest {
            operator_id: "test-operator".into(),
            target,
            standard: FileEraseStandard::NistClear,
            operator_confirmed_target: format!("file {}", path.display()),
            block_size: None,
        }
    }

    #[test]
    fn single_file_nist_clear_erases_verifies_and_deletes() {
        let dir = setup_dir();
        let path = dir.join("doc.txt");
        write_file(&path, 260_000, 0xA5);
        let request = nist_clear_request(path.clone());
        let result = erase(request).unwrap();
        assert!(result.complete);
        assert_eq!(result.files.len(), 1);
        let rec = &result.files[0];
        assert!(rec.verified);
        assert!(rec.deleted);
        assert_eq!(rec.mismatched_sectors, 0);
        assert!(!path.exists(), "file must be gone after erase");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn single_file_purge_3_pass_erases_verifies_and_deletes() {
        let dir = setup_dir();
        let path = dir.join("secret.bin");
        write_file(&path, 700_000, 0x11);
        let mut request = nist_clear_request(path.clone());
        request.standard = FileEraseStandard::Purge3Pass;
        let result = erase(request).unwrap();
        assert!(result.complete);
        assert!(result.files[0].verified && result.files[0].deleted);
        assert!(!path.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn folder_is_erased_recursively_and_removed() {
        let dir = setup_dir();
        let top = dir.join("folder");
        std::fs::create_dir_all(top.join("sub/inner")).unwrap();
        write_file(&top.join("a.txt"), 100_000, 0x22);
        write_file(&top.join("sub/b.bin"), 350_000, 0x33);
        write_file(&top.join("sub/inner/c.log"), 60_000, 0x44);
        let request = EraseRequest {
            operator_id: "test-operator".into(),
            target: EraseTarget::Folder(top.clone()),
            standard: FileEraseStandard::NistClear,
            operator_confirmed_target: format!("folder {}", top.display()),
            block_size: None,
        };
        let result = erase(request).unwrap();
        assert!(result.complete);
        assert_eq!(result.files.len(), 3);
        assert!(result.files.iter().all(|f| f.verified && f.deleted));
        assert_eq!(result.directories_removed, 3);
        assert!(!top.exists(), "folder tree must be removed");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn confirmation_mismatch_is_refused() {
        let dir = setup_dir();
        let path = dir.join("doc.txt");
        write_file(&path, 10_000, 0x55);
        let mut request = nist_clear_request(path.clone());
        request.operator_confirmed_target = "file some/other/path".into();
        let err = erase(request).unwrap_err();
        assert_eq!(err, EraseError::ConfirmationMismatch);
        assert!(path.exists(), "nothing may be touched on refused wipe");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_target_is_reported() {
        let dir = setup_dir();
        let path = dir.join("nope.txt");
        let request = nist_clear_request(path.clone());
        let err = erase(request).unwrap_err();
        assert!(matches!(err, EraseError::NotFound(_)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(windows)]
    #[test]
    fn currently_open_file_is_refused() {
        let dir = setup_dir();
        let path = dir.join("locked.txt");
        write_file(&path, 40_000, 0x66);
        let held = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        let request = nist_clear_request(path.clone());
        let err = erase(request).unwrap_err();
        assert!(
            matches!(err, EraseError::ExclusivityDenied(_)),
            "wipe of a file held by another process must be refused, got {err:?}"
        );
        drop(held);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_folder_has_nothing_to_erase() {
        let dir = setup_dir();
        let empty = dir.join("empty");
        std::fs::create_dir_all(empty.join("child")).unwrap();
        let request = EraseRequest {
            operator_id: "test-operator".into(),
            target: EraseTarget::Folder(empty.clone()),
            standard: FileEraseStandard::NistClear,
            operator_confirmed_target: format!("folder {}", empty.display()),
            block_size: None,
        };
        let err = erase(request).unwrap_err();
        assert!(matches!(err, EraseError::NotFound(_)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn log_module_is_usable() {
        let _ = log::now_utc_rfc3339();
        assert!(log::now_utc_rfc3339().contains('Z') || log::now_utc_rfc3339().contains('+'));
    }
}
