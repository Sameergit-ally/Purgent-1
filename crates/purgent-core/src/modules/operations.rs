use std::path::{Path, PathBuf};

use super::drive_eraser::{WipeError, WipeRequest, WipeResult};
use super::file_eraser::{EraseError, EraseRequest, FileEraseResult};
use super::progress::ProgressFn;
use super::recovery::{CarveError, CarveRun};

pub use super::reporting::Report;

#[derive(Debug)]
pub struct WipeOutcome {
    pub result: WipeResult,
    pub report: Report,
    pub report_json_path: PathBuf,
    pub report_pdf_path: PathBuf,
}

#[derive(Debug)]
pub struct EraseOutcome {
    pub result: FileEraseResult,
    pub report: Report,
    pub report_json_path: PathBuf,
    pub report_pdf_path: PathBuf,
}

#[derive(Debug)]
pub struct CarveOutcome {
    pub run: CarveRun,
    pub report: Report,
    pub report_json_path: PathBuf,
    pub report_pdf_path: PathBuf,
}

pub fn run_wipe(
    request: WipeRequest,
    report_dir: &Path,
    signing_key: &[u8],
) -> Result<WipeOutcome, WipeError> {
    run_wipe_with_progress(request, report_dir, signing_key, &mut |_| {})
}

pub fn run_wipe_with_progress(
    request: WipeRequest,
    report_dir: &Path,
    signing_key: &[u8],
    progress: ProgressFn,
) -> Result<WipeOutcome, WipeError> {
    let result = super::drive_eraser::wipe_with_progress(request, progress)?;
    let report = super::reporting::build_wipe_report(&result, signing_key);
    let (report_json_path, report_pdf_path) =
        super::reporting::save_report(&report, &report_dir.to_path_buf())
            .map_err(WipeError::Reporting)?;
    super::log::log(
        "reported",
        &format!(
            "operation={} report_signed=true json={} pdf={}",
            result.operation_id,
            report_json_path.display(),
            report_pdf_path.display()
        ),
    );
    Ok(WipeOutcome {
        result,
        report,
        report_json_path,
        report_pdf_path,
    })
}

pub fn run_erase(
    request: EraseRequest,
    report_dir: &Path,
    signing_key: &[u8],
) -> Result<EraseOutcome, EraseError> {
    run_erase_with_progress(request, report_dir, signing_key, &mut |_| {})
}

pub fn run_erase_with_progress(
    request: EraseRequest,
    report_dir: &Path,
    signing_key: &[u8],
    progress: ProgressFn,
) -> Result<EraseOutcome, EraseError> {
    let result = super::file_eraser::erase_with_progress(request, progress)?;
    let report = super::reporting::build_file_erase_report(&result, signing_key);
    let (report_json_path, report_pdf_path) =
        super::reporting::save_report(&report, &report_dir.to_path_buf())
            .map_err(EraseError::Irrecoverable)?;
    super::log::log(
        "reported",
        &format!(
            "operation={} report_signed=true json={} pdf={}",
            result.operation_id,
            report_json_path.display(),
            report_pdf_path.display()
        ),
    );
    Ok(EraseOutcome {
        result,
        report,
        report_json_path,
        report_pdf_path,
    })
}

pub fn run_carve(
    source: &Path,
    output_dir: &Path,
    report_dir: &Path,
    signing_key: &[u8],
) -> Result<CarveOutcome, CarveError> {
    run_carve_with_progress(source, output_dir, report_dir, signing_key, &mut |_| {})
}

pub fn run_carve_with_progress(
    source: &Path,
    output_dir: &Path,
    report_dir: &Path,
    signing_key: &[u8],
    progress: ProgressFn,
) -> Result<CarveOutcome, CarveError> {
    let run = super::recovery::carve_source_with_progress(source, output_dir, progress)?;
    let report = super::reporting::build_recovery_report(&run, signing_key);
    let (report_json_path, report_pdf_path) =
        super::reporting::save_report(&report, &report_dir.to_path_buf())
            .map_err(CarveError::Destination)?;
    super::log::log(
        "reported",
        &format!(
            "operation={} report_signed=true json={} pdf={}",
            run.operation_id,
            report_json_path.display(),
            report_pdf_path.display()
        ),
    );
    Ok(CarveOutcome {
        run,
        report,
        report_json_path,
        report_pdf_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::config::WipeStandard;
    use crate::modules::drive_eraser::{VerificationStatus, WipeTarget};
    use crate::modules::file_eraser::{EraseTarget, FileEraseStandard};

    const TEST_KEY: &[u8] = b"operations-test-key-000000000000000000000000000";

    fn sample_wipe_image(dir: &Path) -> PathBuf {
        let path = dir.join("wipe.img");
        std::fs::write(&path, vec![0x55u8; 512 * 8]).unwrap();
        path
    }

    #[test]
    fn wipe_operation_emits_signed_report() {
        let dir = std::env::temp_dir().join(format!("purgent-op-wipe-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let image = sample_wipe_image(&dir);
        let target = WipeTarget::ImageFile(image.clone());
        let request = WipeRequest {
            operator_id: "opr-1".into(),
            target,
            standard: WipeStandard::Nist800_88Clear,
            operator_confirmed_target: format!("image file {}", image.display()),
            block_size: None,
            fallback_acknowledged: false,
        };
        let report_dir = dir.join("reports");
        let outcome = run_wipe(request, &report_dir, TEST_KEY).unwrap();
        assert_eq!(
            outcome.result.verification.as_ref().unwrap().status,
            VerificationStatus::Passed
        );
        assert_eq!(outcome.result.complete, true);
        assert_eq!(outcome.report.operation_type, "secure_erase");
        let json = std::fs::read(&outcome.report_json_path).unwrap();
        assert!(
            crate::modules::reporting::verify_report(&json, TEST_KEY),
            "saved wipe report must verify"
        );
        assert!(outcome.report_pdf_path.exists());
        let mut tampered = json.clone();
        let last = tampered.len() - 1;
        tampered[last] = if tampered[last] == b'}' { b'x' } else { b'}' };
        assert!(!crate::modules::reporting::verify_report(
            &tampered, TEST_KEY
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dashboard_end_to_end_operate_verify_view() {
        // mirrors the Phase 8 checkpoint: pick target -> operate -> verify -> view report
        let dir = std::env::temp_dir().join(format!("purgent-dash-e2e-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let image = sample_wipe_image(&dir);
        let report_dir = dir.join("reports");

        let target = WipeTarget::ImageFile(image.clone());
        let request = WipeRequest {
            operator_id: "dash-operator".into(),
            target,
            standard: WipeStandard::Nist800_88Clear,
            operator_confirmed_target: format!("image file {}", image.display()),
            block_size: None,
            fallback_acknowledged: false,
        };
        let updates = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let updates_clone = updates.clone();
        let mut prog: super::super::progress::ProgressFn<'_> = &mut |u| {
            updates_clone.lock().unwrap().push(u);
        };
        let outcome = run_wipe_with_progress(request, &report_dir, TEST_KEY, &mut prog).unwrap();

        assert_eq!(
            outcome.result.verification.as_ref().unwrap().status,
            VerificationStatus::Passed,
            "operate step must verify pass"
        );

        let json = std::fs::read(&outcome.report_json_path).unwrap();
        assert!(
            crate::modules::reporting::verify_report(&json, TEST_KEY),
            "verify step: saved report must be tamper-evident valid"
        );
        let view: crate::modules::reporting::Report =
            serde_json::from_slice(&json).expect("view step: report deserializes");
        assert_eq!(view.operation_type, "secure_erase");
        assert_eq!(view.operator_id, "dash-operator");
        assert!(!view.report_hash.is_empty());
        assert_eq!(view.signature_alg, "HMAC-SHA256");
        assert!(matches!(
            view.verification.as_ref().unwrap().status,
            VerificationStatus::Passed
        ));

        let progress = updates.lock().unwrap();
        assert!(!progress.is_empty());
        assert!(progress.iter().all(|u| u.total_bytes == 4096));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn wipe_operation_refuses_confirmation_mismatch() {
        let dir =
            std::env::temp_dir().join(format!("purgent-op-mismatch-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let image = sample_wipe_image(&dir);
        let request = WipeRequest {
            operator_id: "opr-1".into(),
            target: WipeTarget::ImageFile(image.clone()),
            standard: WipeStandard::Nist800_88Clear,
            operator_confirmed_target: "image file wrong.img".into(),
            block_size: None,
            fallback_acknowledged: false,
        };
        let err = run_wipe(request, &dir.join("reports"), TEST_KEY).unwrap_err();
        assert!(matches!(err, WipeError::ConfirmationMismatch));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn erase_operation_emits_signed_report() {
        let dir = std::env::temp_dir().join(format!("purgent-op-erase-{}", uuid::Uuid::new_v4()));
        let folder = dir.join("folder");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("a.txt"), vec![0x11; 2048]).unwrap();
        std::fs::write(folder.join("b.bin"), vec![0x22; 4096]).unwrap();
        let request = EraseRequest {
            operator_id: "opr-1".into(),
            target: EraseTarget::Folder(folder.clone()),
            standard: FileEraseStandard::Purge3Pass,
            operator_confirmed_target: format!("folder {}", folder.display()),
            block_size: None,
        };
        let outcome = run_erase(request, &dir.join("reports"), TEST_KEY).unwrap();
        assert!(outcome.result.all_verified_and_deleted());
        let json = std::fs::read(&outcome.report_json_path).unwrap();
        assert!(crate::modules::reporting::verify_report(&json, TEST_KEY));
        assert_eq!(outcome.report.operation_type, "file_erase");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn carve_operation_emits_signed_report() {
        let dir = std::env::temp_dir().join(format!("purgent-op-carve-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("out")).unwrap();
        let mut image = Vec::new();
        image.extend_from_slice(b"$junk$");
        image.extend_from_slice(b"GIF89a\x01\x00\x01\x00\x80\x00\x00\x00\x00\x00\x00\x2C\x00\x00\x00\x00\x01\x00\x01\x00\x00\x02\x02\x44\x01\x00\x3B");
        image.extend_from_slice(b"%%tail%%");
        let image_path = dir.join("c.img");
        std::fs::write(&image_path, &image).unwrap();
        let outcome = run_carve(
            &image_path,
            &dir.join("out"),
            &dir.join("reports"),
            TEST_KEY,
        )
        .unwrap();
        assert_eq!(outcome.run.files.len(), 1, "one gif carved");
        let json = std::fs::read(&outcome.report_json_path).unwrap();
        assert!(crate::modules::reporting::verify_report(&json, TEST_KEY));
        assert_eq!(outcome.report.operation_type, "file_recovery");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn wipe_with_progress_emits_progress_updates() {
        let dir = std::env::temp_dir().join(format!("purgent-op-prog-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let image = sample_wipe_image(&dir);
        let target = WipeTarget::ImageFile(image.clone());
        let request = WipeRequest {
            operator_id: "opr-1".into(),
            target,
            standard: WipeStandard::Nist800_88Clear,
            operator_confirmed_target: format!("image file {}", image.display()),
            block_size: None,
            fallback_acknowledged: false,
        };
        let report_dir = dir.join("reports");
        let updates = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let updates_clone = updates.clone();
        let mut prog: super::super::progress::ProgressFn<'_> = &mut |upd| {
            updates_clone.lock().unwrap().push(upd);
        };
        let outcome = run_wipe_with_progress(request, &report_dir, TEST_KEY, &mut prog).unwrap();
        assert_eq!(outcome.result.complete, true);
        let collected = updates.lock().unwrap();
        assert!(
            !collected.is_empty(),
            "must emit at least one progress update"
        );
        assert!(collected.iter().any(|u| u.operation_type == "secure_erase"),);
        assert!(
            collected.last().unwrap().fraction() > 0.0,
            "fraction must advance"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reports_produced_by_different_operations_verify_with_their_own_key() {
        let dir = std::env::temp_dir().join(format!("purgent-op-key-{}", uuid::Uuid::new_v4()));
        let folder = dir.join("f");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("x.bin"), vec![0x33; 1024]).unwrap();
        let request = EraseRequest {
            operator_id: "opr-2".into(),
            target: EraseTarget::Folder(folder),
            standard: FileEraseStandard::NistClear,
            operator_confirmed_target: format!("folder {}", dir.join("f").display()),
            block_size: None,
        };
        let outcome = run_erase(request, &dir.join("reports"), TEST_KEY).unwrap();
        let json = std::fs::read(outcome.report_json_path).unwrap();
        assert!(!crate::modules::reporting::verify_report(
            &json,
            b"the-wrong-key............."
        ));
        assert!(crate::modules::reporting::verify_report(&json, TEST_KEY));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
