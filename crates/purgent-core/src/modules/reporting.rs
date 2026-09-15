use std::fs;
use std::path::PathBuf;

use uuid::Uuid;

use super::classification::FileCategory;
use super::drive_eraser::{VerificationResult, VerificationStatus, WipeResult};
use super::file_eraser::{EraseFileRecord, EraseVerificationStatus, FileEraseResult};
use super::hashing::sha256_hex;
use super::recovery::CarveRun;
use super::signing::hmac_sha256_hex;
use super::storage::hpa_dco::{HpaDcoRemoval, HpaDcoState};
use super::trace_scrubber::TraceScrubRecord;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Report {
    pub report_id: String,
    pub operation_type: String,
    pub operation_id: String,
    pub operator_id: String,
    pub target: String,
    pub standard_id: String,
    pub standard_label: String,
    pub capacity_bytes: u64,
    pub start_time: String,
    pub finish_time: String,
    pub verification: Option<VerificationResult>,
    pub skipped_sector_count: u64,
    pub method: Option<String>,
    pub evidence_hash: Option<String>,
    pub hpa_dco: Option<HpaDcoState>,
    pub hpa_dco_removal: Option<HpaDcoRemoval>,
    pub trace_scrub: Option<TraceScrubRecord>,
    pub categories: Option<Vec<FileCategoryCount>>,
    pub report_hash: String,
    pub signature_alg: String,
    pub signature: String,
}

impl Default for Report {
    fn default() -> Self {
        Report {
            report_id: String::new(),
            operation_type: String::new(),
            operation_id: String::new(),
            operator_id: String::new(),
            target: String::new(),
            standard_id: String::new(),
            standard_label: String::new(),
            capacity_bytes: 0,
            start_time: String::new(),
            finish_time: String::new(),
            verification: None,
            skipped_sector_count: 0,
            method: None,
            evidence_hash: None,
            hpa_dco: None,
            hpa_dco_removal: None,
            trace_scrub: None,
            categories: None,
            report_hash: String::new(),
            signature_alg: String::new(),
            signature: String::new(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileCategoryCount {
    pub category: FileCategory,
    pub label: String,
    pub count: usize,
}

pub fn category_counts(files: &[super::recovery::RecoveredFile]) -> Vec<FileCategoryCount> {
    super::classification::aggregate_categories(files)
        .into_iter()
        .map(|(category, count)| FileCategoryCount {
            category,
            label: category.label().to_string(),
            count,
        })
        .collect()
}

const SIGNATURE_ALG: &str = "HMAC-SHA256";

fn report_payload_map(report: &Report) -> serde_json::Map<String, serde_json::Value> {
    let mut map = serde_json::to_value(report)
        .expect("report serializes")
        .as_object()
        .cloned()
        .expect("report is an object");
    map.remove("report_hash");
    map.remove("signature_alg");
    map.remove("signature");
    map
}

fn payload_json(report: &Report) -> String {
    serde_json::to_string(&serde_json::Value::Object(report_payload_map(report)))
        .expect("payload serializes")
}

fn canonical_payload_bytes(report: &Report) -> Vec<u8> {
    payload_json(report).into_bytes()
}

pub(crate) fn sign_report(report: &mut Report, key: &[u8]) {
    let payload = canonical_payload_bytes(report);
    report.report_hash = sha256_hex(&payload);
    report.signature_alg = SIGNATURE_ALG.to_string();
    report.signature = hmac_sha256_hex(key, &canonical_payload_bytes(report));
}

pub fn build_wipe_report(result: &WipeResult, key: &[u8]) -> Report {
    let verification = result.verification.as_ref().map(|v| VerificationResult {
        status: v.status,
        bytes_verified: v.bytes_verified,
        mismatched_sectors: v.mismatched_sectors,
        detail: v.detail.clone(),
    });
    let mut report = Report {
        report_id: Uuid::new_v4().to_string(),
        operation_type: "secure_erase".to_string(),
        operation_id: result.operation_id.clone(),
        operator_id: result.operator_id.clone(),
        target: result.target.clone(),
        standard_id: serde_json::to_value(result.standard)
            .expect("standard serializes")
            .as_str()
            .unwrap_or("unknown")
            .to_string(),
        standard_label: result.standard_label.clone(),
        capacity_bytes: result.capacity_bytes,
        start_time: result.started_at.clone(),
        finish_time: result.finished_at.clone(),
        verification,
        skipped_sector_count: result.skipped_sectors.len() as u64,
        method: Some(result.method.label().to_string()),
        evidence_hash: result.evidence_hash.clone(),
        hpa_dco: result.hpa_dco.clone(),
        hpa_dco_removal: result.hpa_dco_removal.clone(),
        trace_scrub: None,
        categories: None,
        report_hash: String::new(),
        signature_alg: String::new(),
        signature: String::new(),
    };
    sign_report(&mut report, key);
    report
}

fn aggregate_verification(
    status: EraseVerificationStatus,
    files: &[EraseFileRecord],
) -> VerificationResult {
    let bytes_verified = files.iter().map(|f| f.bytes_erased).sum();
    let mismatched = files.iter().map(|f| f.mismatched_sectors).sum();
    let verified = files.iter().filter(|f| f.verified).count();
    VerificationResult {
        status: if status == EraseVerificationStatus::Passed {
            VerificationStatus::Passed
        } else {
            VerificationStatus::Failed
        },
        bytes_verified,
        mismatched_sectors: mismatched,
        detail: format!(
            "files={} verified={} of {}",
            files.len(),
            verified,
            files.len()
        ),
    }
}

pub fn build_file_erase_report(result: &FileEraseResult, key: &[u8]) -> Report {
    let status = if result.all_verified_and_deleted() {
        EraseVerificationStatus::Passed
    } else {
        EraseVerificationStatus::Failed
    };
    let mut report = Report {
        report_id: Uuid::new_v4().to_string(),
        operation_type: "file_erase".to_string(),
        operation_id: result.operation_id.clone(),
        operator_id: result.operator_id.clone(),
        target: result.target.clone(),
        standard_id: serde_json::to_value(result.standard)
            .expect("standard serializes")
            .as_str()
            .unwrap_or("unknown")
            .to_string(),
        standard_label: result.standard_label.clone(),
        capacity_bytes: result.files.iter().map(|f| f.bytes_erased).sum(),
        start_time: result.started_at.clone(),
        finish_time: result.finished_at.clone(),
        verification: Some(aggregate_verification(status, &result.files)),
        skipped_sector_count: 0,
        method: None,
        evidence_hash: None,
        hpa_dco: None,
        hpa_dco_removal: None,
        trace_scrub: result.trace_scrub.clone(),
        categories: None,
        report_hash: String::new(),
        signature_alg: String::new(),
        signature: String::new(),
    };
    sign_report(&mut report, key);
    report
}

pub fn build_recovery_report(run: &CarveRun, key: &[u8]) -> Report {
    let bytes_verified: u64 = run.files.iter().map(|f| f.size_bytes).sum();
    let verification = VerificationResult {
        status: if run.files.is_empty() {
            VerificationStatus::Failed
        } else {
            VerificationStatus::Passed
        },
        bytes_verified,
        mismatched_sectors: 0,
        detail: format!(
            "files_recovered={} source_scanned_read_only=true bytes={} hashes_recorded=true",
            run.files.len(),
            run.scanned_bytes
        ),
    };
    let mut report = Report {
        report_id: Uuid::new_v4().to_string(),
        operation_type: "file_recovery".to_string(),
        operation_id: run.operation_id.clone(),
        operator_id: String::new(),
        target: run.source.clone(),
        standard_id: "carve_signature_read_only".to_string(),
        standard_label: "Signature-based carving (read-only source)".to_string(),
        capacity_bytes: run.scanned_bytes,
        start_time: run.started_at.clone(),
        finish_time: run.finished_at.clone(),
        verification: Some(verification),
        skipped_sector_count: 0,
        method: None,
        evidence_hash: None,
        hpa_dco: None,
        hpa_dco_removal: None,
        trace_scrub: None,
        categories: {
            let counts = category_counts(&run.files);
            if counts.is_empty() {
                None
            } else {
                Some(counts)
            }
        },
        report_hash: String::new(),
        signature_alg: String::new(),
        signature: String::new(),
    };
    sign_report(&mut report, key);
    report
}

pub fn report_to_json_string(report: &Report) -> String {
    serde_json::to_string(report).expect("report serializes")
}

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            other => out.push(other),
        }
    }
    out
}

fn tag(name: &str, value: &str) -> String {
    format!("<{name}>{}</{name}>", xml_escape(value))
}

/// Renders the signed report as an audit-friendly XML certificate. The payload
/// fields mirror the canonical signed JSON so the XML can be cross-checked against
/// the stored `report_hash` / `signature`.
pub fn export_xml(report: &Report) -> String {
    let mut buf = String::new();
    buf.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    buf.push_str("<purgent_report>\n");
    buf.push_str(&tag("report_id", &format!("{}", report.report_id)));
    buf.push_str(&tag("operation_type", &report.operation_type));
    buf.push_str(&tag("operation_id", &report.operation_id));
    buf.push_str(&tag("operator_id", &report.operator_id));
    buf.push_str(&tag("target", &report.target));
    buf.push_str(&tag("standard_id", &report.standard_id));
    buf.push_str(&tag("standard_label", &report.standard_label));
    buf.push_str(&tag("capacity_bytes", &report.capacity_bytes.to_string()));
    buf.push_str(&tag("start_time", &report.start_time));
    buf.push_str(&tag("finish_time", &report.finish_time));
    if let Some(method) = &report.method {
        buf.push_str(&tag("method", method));
    }
    if let Some(v) = &report.verification {
        buf.push_str("<verification>\n");
        buf.push_str(&format!(
            "  {}\n",
            tag(
                "status",
                &serde_json::to_string(&v.status).unwrap_or_default()
            )
        ));
        buf.push_str(&format!(
            "  {}\n",
            tag("bytes_verified", &v.bytes_verified.to_string())
        ));
        buf.push_str(&format!(
            "  {}\n",
            tag("mismatched_sectors", &v.mismatched_sectors.to_string())
        ));
        buf.push_str(&format!("  {}\n", tag("detail", &v.detail)));
        buf.push_str("</verification>\n");
    }
    if let Some(evidence) = &report.evidence_hash {
        buf.push_str(&tag("evidence_sha256", evidence));
    }
    if let Some(hpa) = &report.hpa_dco {
        buf.push_str("<hpa_dco>\n");
        buf.push_str(&format!(
            "  {}\n",
            tag("hpa_present", &hpa.hpa_present.to_string())
        ));
        buf.push_str(&format!(
            "  {}\n",
            tag("dco_present", &hpa.dco_present.to_string())
        ));
        buf.push_str(&format!(
            "  {}\n",
            tag("removable", &hpa.removable.to_string())
        ));
        buf.push_str(&format!(
            "  {}\n",
            tag("max_current_lba", &hpa.max_current_lba.to_string())
        ));
        buf.push_str(&format!(
            "  {}\n",
            tag("max_native_lba", &hpa.max_native_lba.to_string())
        ));
        buf.push_str("</hpa_dco>\n");
    }
    if let Some(removal) = &report.hpa_dco_removal {
        buf.push_str("<hpa_dco_removal>\n");
        buf.push_str(&format!(
            "  {}\n",
            tag("attempted", &removal.attempted.to_string())
        ));
        buf.push_str(&format!(
            "  {}\n",
            tag("hpa_clear_sent", &removal.hpa_clear_sent.to_string())
        ));
        buf.push_str(&format!(
            "  {}\n",
            tag("dco_reset_sent", &removal.dco_reset_sent.to_string())
        ));
        buf.push_str(&format!(
            "  {}\n",
            tag("removed", &removal.removed.to_string())
        ));
        buf.push_str(&format!("  {}\n", tag("detail", &removal.detail)));
        buf.push_str("</hpa_dco_removal>\n");
    }
    if let Some(scrub) = &report.trace_scrub {
        buf.push_str("<trace_scrub>\n");
        buf.push_str(&format!("  {}\n", tag("path", &scrub.file_path)));
        buf.push_str(&format!("  {}\n", tag("scrubbed_at", &scrub.scrubbed_at)));
        buf.push_str(&format!(
            "  {}\n",
            tag("ok_actions", &scrub.ok_count().to_string())
        ));
        buf.push_str(&format!(
            "  {}\n",
            tag(
                "best_effort_actions",
                &scrub.best_effort_count().to_string()
            )
        ));
        buf.push_str(&format!(
            "  {}\n",
            tag("skipped_actions", &scrub.skipped_count().to_string())
        ));
        buf.push_str("</trace_scrub>\n");
    }
    if let Some(cats) = &report.categories {
        if !cats.is_empty() {
            buf.push_str("<categories>\n");
            for c in cats {
                buf.push_str(&format!(
                    "  <category name=\"{}\" count=\"{}\"/>\n",
                    xml_escape(&c.label),
                    c.count
                ));
            }
            buf.push_str("</categories>\n");
        }
    }
    buf.push_str(&tag(
        "verification_status",
        &report
            .verification
            .as_ref()
            .map(|v| serde_json::to_string(&v.status).unwrap_or_default())
            .unwrap_or_else(|| "pending_hardware".into()),
    ));
    buf.push_str(&tag(
        "skipped_sector_count",
        &report.skipped_sector_count.to_string(),
    ));
    buf.push_str(&tag("report_hash", &report.report_hash));
    buf.push_str(&tag("signature_alg", &report.signature_alg));
    buf.push_str(&tag("signature", &report.signature));
    buf.push_str("</purgent_report>\n");
    buf
}

/// Saves the JSON + PDF certificate, and records the XML export path.
pub fn save_report_xml(report: &Report, output_dir: &PathBuf) -> Result<PathBuf, String> {
    fs::create_dir_all(output_dir).map_err(|e| format!("cannot create report dir: {e}"))?;
    let xml_path = output_dir.join(format!("report-{}.xml", report.report_id));
    fs::write(&xml_path, export_xml(report))
        .map_err(|e| format!("cannot write report xml: {e}"))?;
    Ok(xml_path)
}

pub fn save_report(report: &Report, output_dir: &PathBuf) -> Result<(PathBuf, PathBuf), String> {
    fs::create_dir_all(output_dir).map_err(|e| format!("cannot create report dir: {e}"))?;
    let json_path = output_dir.join(format!("report-{}.json", report.report_id));
    let pdf_path = output_dir.join(format!("report-{}.pdf", report.report_id));
    fs::write(&json_path, report_to_json_string(report))
        .map_err(|e| format!("cannot write report json: {e}"))?;
    fs::write(&pdf_path, pdf_certificate(report))
        .map_err(|e| format!("cannot write report pdf: {e}"))?;
    Ok((json_path, pdf_path))
}

pub fn pdf_certificate(report: &Report) -> Vec<u8> {
    let title = match report.operation_type.as_str() {
        "secure_erase" => "PURGENT - SECURE ERASURE CERTIFICATE".to_string(),
        "file_erase" => "PURGENT - FILE ERASURE CERTIFICATE".to_string(),
        "file_recovery" => "PURGENT - RECOVERY CERTIFICATE".to_string(),
        _ => "PURGENT - OPERATION CERTIFICATE".to_string(),
    };
    let lines: Vec<String> = [
        title,
        format!("Report ID:    {}", report.report_id),
        format!("Operation ID: {}", report.operation_id),
        format!("Operator ID:  {}", report.operator_id),
        format!("Target:       {}", report.target),
        format!("Standard:     {}", report.standard_label),
        format!("Capacity:     {} bytes", report.capacity_bytes),
        format!("Started:      {}", report.start_time),
        format!("Finished:     {}", report.finish_time),
        format!(
            "Verification: {}",
            serde_json::to_string(&report.verification).unwrap_or_default()
        ),
        format!("Skipped sects:{}", report.skipped_sector_count),
        format!("Report SHA256:{}", report.report_hash),
        format!(
            "Signature:    {} {}",
            report.signature_alg, report.signature
        ),
    ]
    .into_iter()
    .collect();

    let content = lines
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let mut escaped = String::with_capacity(l.len());
            for c in l.chars() {
                match c {
                    '\\' => escaped.push_str("\\\\"),
                    '(' => escaped.push_str("\\("),
                    ')' => escaped.push_str("\\)"),
                    _ => escaped.push(c),
                }
            }
            format!("BT /F1 8 Tf 40 {} Td ({}) Tj ET\n", 760 - i * 18, escaped)
        })
        .collect::<String>()
        .into_bytes();

    let objects: Vec<Vec<u8>> = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>".to_vec(),
        {
            let mut buf = b"<< /Length ".to_vec();
            buf.extend_from_slice(content.len().to_string().as_bytes());
            buf.extend_from_slice(b" >>\nstream\n");
            buf.extend_from_slice(&content);
            buf.extend_from_slice(b"endstream");
            buf
        },
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec(),
    ];
    let mut offsets: Vec<usize> = vec![];
    let mut out: Vec<u8> = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    for (i, _) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(&objects[i]);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
    for off in &offsets {
        out.extend_from_slice(format!("{:010} 00000 n \n", off).as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF",
            objects.len() + 1,
            xref
        )
        .as_bytes(),
    );
    out
}

pub fn verify_report(bytes: &[u8], key: &[u8]) -> bool {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return false;
    };
    let Some(stored_hash) = value.get("report_hash").and_then(|v| v.as_str()) else {
        return false;
    };
    let Some(stored_sig) = value.get("signature").and_then(|v| v.as_str()) else {
        return false;
    };
    let Some(stored_alg) = value.get("signature_alg").and_then(|v| v.as_str()) else {
        return false;
    };
    if stored_alg != SIGNATURE_ALG {
        return false;
    }
    let Some(mut map) = value.as_object().cloned() else {
        return false;
    };
    map.remove("report_hash");
    map.remove("signature_alg");
    map.remove("signature");
    let payload = serde_json::to_string(&serde_json::Value::Object(map)).unwrap_or_default();
    let recomputed_hash = sha256_hex(payload.as_bytes());
    if stored_hash != recomputed_hash {
        return false;
    }
    stored_sig == hmac_sha256_hex(key, payload.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::config::WipeStandard;
    use crate::modules::drive_eraser::{VerificationStatus, WipeResult};
    use crate::modules::signing::hmac_sha256_hex;

    const TEST_KEY: &[u8] = b"phase-7-test-key-00000000000000000000000000";

    fn sample_wipe_result() -> WipeResult {
        WipeResult {
            operation_id: "op-1".into(),
            target: "image file C:\\test\\disk.img".into(),
            standard: WipeStandard::Nist800_88Clear,
            standard_label: WipeStandard::Nist800_88Clear.label().into(),
            capacity_bytes: 512,
            started_at: "2026-09-13T10:00:00.000Z".into(),
            finished_at: "2026-09-13T10:00:01.000Z".into(),
            operator_id: "opr-1".into(),
            verification: Some(VerificationResult {
                status: VerificationStatus::Passed,
                bytes_verified: 512,
                mismatched_sectors: 0,
                detail: "read-back verified 512 bytes".into(),
            }),
            skipped_sectors: vec![],
            complete: true,
            method: crate::modules::config::WipeMethod::Overwrite,
            hpa_dco: None,
            hpa_dco_removal: None,
            evidence_hash: None,
            fallback_reason: None,
        }
    }

    fn signed_report() -> Report {
        let mut report = build_wipe_report(&sample_wipe_result(), TEST_KEY);
        report.report_id = "fixed-report-id".into();
        sign_report(&mut report, TEST_KEY);
        report
    }

    #[test]
    fn wipe_report_signature_and_hash_verify() {
        let report = signed_report();
        assert_eq!(report.signature_alg, "HMAC-SHA256");
        assert_eq!(report.signature.len(), 64, "HMAC-SHA256 hex is 64 chars");
        let json = report_to_json_string(&report);
        assert!(verify_report(json.as_bytes(), TEST_KEY));
    }

    #[test]
    fn wipe_report_tamper_detected() {
        let report = signed_report();
        let mut json = report_to_json_string(&report).into_bytes();
        let len = json.len();
        json[len - 3] = if json[len - 3] == b'}' { b'x' } else { b'}' };
        assert!(!verify_report(&json, TEST_KEY));
    }

    #[test]
    fn report_signed_by_wrong_key_is_rejected() {
        let report = signed_report();
        let json = report_to_json_string(&report);
        assert!(!verify_report(
            json.as_bytes(),
            b"different key material .........."
        ));
        assert!(!verify_report(json.as_bytes(), &[]));
    }

    #[test]
    fn file_erase_report_signs_and_verifies() {
        let result = FileEraseResult {
            operation_id: "fe-1".into(),
            target: "folder C:\\data\\vault".into(),
            standard: crate::modules::file_eraser::FileEraseStandard::Purge3Pass,
            standard_label: crate::modules::file_eraser::FileEraseStandard::Purge3Pass
                .label()
                .into(),
            started_at: "2026-09-13T10:00:00.000Z".into(),
            finished_at: "2026-09-13T10:00:02.000Z".into(),
            operator_id: "opr-1".into(),
            files: vec![EraseFileRecord {
                path: "C:\\data\\vault\\a.txt".into(),
                bytes_erased: 512,
                mismatched_sectors: 0,
                verified: true,
                deleted: true,
            }],
            directories_removed: 1,
            complete: true,
            trace_scrub: None,
        };
        let report = build_file_erase_report(&result, TEST_KEY);
        let json = report_to_json_string(&report);
        assert!(verify_report(json.as_bytes(), TEST_KEY));
        assert_eq!(report.operation_type, "file_erase");
        assert_eq!(
            report.verification.as_ref().unwrap().status,
            VerificationStatus::Passed,
            "all files verified and deleted => passed"
        );
    }

    #[test]
    fn recovery_report_signs_and_verifies() {
        let run = CarveRun {
            operation_id: "rv-1".into(),
            source: "C:\\evidence\\disk.img".into(),
            output_dir: "C:\\evidence\\out".into(),
            scanned_bytes: 123456,
            files: vec![],
            started_at: "2026-09-13T10:00:00.000Z".into(),
            finished_at: "2026-09-13T10:00:01.000Z".into(),
        };
        let report = build_recovery_report(&run, TEST_KEY);
        let json = report_to_json_string(&report);
        assert!(verify_report(json.as_bytes(), TEST_KEY));
        assert_eq!(report.operation_type, "file_recovery");
    }

    #[test]
    fn hmac_over_canonical_payload_is_deterministic() {
        let a = build_wipe_report(&sample_wipe_result(), TEST_KEY);
        let b = build_wipe_report(&sample_wipe_result(), TEST_KEY);
        assert_ne!(a.report_id, b.report_id, "fresh report ids");
        let payload_a = canonical_payload_bytes(&a);
        let payload_b = canonical_payload_bytes(&b);
        let expected_a = hmac_sha256_hex(TEST_KEY, &payload_a);
        assert_eq!(
            a.signature, expected_a,
            "signature recomputes from canonical payload"
        );
        assert_eq!(payload_a.len(), payload_b.len());
    }

    #[test]
    fn pdf_certificate_is_deterministic_and_readable() {
        let a = pdf_certificate(&signed_report());
        let b = pdf_certificate(&signed_report());
        assert_eq!(
            a, b,
            "PDF output must be deterministic for identical inputs"
        );
        assert!(a.starts_with(b"%PDF-1.4"));
        assert!(a.ends_with(b"%%EOF"));
        let text = String::from_utf8_lossy(&a);
        assert!(text.contains("CERTIFICATE"));
        assert!(text.contains("Signature:    HMAC-SHA256 "));
    }

    #[test]
    fn xml_export_is_well_formed_certificate() {
        let report = signed_report();
        let xml = export_xml(&report);
        assert!(xml.starts_with("<?xml version=\"1.0\""));
        assert!(xml.contains("<purgent_report>"));
        assert!(xml.contains(&format!("<report_id>{}</report_id>", report.report_id)));
        assert!(xml.contains(&format!(
            "<report_hash>{}</report_hash>",
            report.report_hash
        )));
        assert!(xml.contains(&format!("<signature>{}</signature>", report.signature)));
        assert!(xml.contains("<verification>"));
        assert!(xml.ends_with("</purgent_report>\n"));
        assert!(
            !xml.contains("&amp;invalid"),
            "unknown fields must be absent"
        );
    }

    #[test]
    fn xml_export_escapes_xml_special_characters() {
        let mut report = signed_report();
        report.target = "folder C:\\data & <vault>'\"".into();
        let xml = export_xml(&report);
        assert!(xml.contains("C:\\data &amp; &lt;vault&gt;&apos;&quot;"));
        assert!(!xml.contains("<vault>"));
    }

    #[test]
    fn save_report_xml_writes_file() {
        let dir = std::env::temp_dir().join(format!("purgent-xml-{}", uuid::Uuid::new_v4()));
        let report = signed_report();
        let xml_path = save_report_xml(&report, &dir).unwrap();
        assert!(xml_path.exists());
        let contents = std::fs::read_to_string(&xml_path).unwrap();
        assert!(contents.contains(&format!("<report_id>{}</report_id>", report.report_id)));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
