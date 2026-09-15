use std::time::Duration;

use serde_json::json;

use super::persistence::CaseDb;
use super::reporting::Report;

pub const SUPABASE_URL_ENV: &str = "PURGENT_SUPABASE_URL";
pub const SUPABASE_ANON_KEY_ENV: &str = "PURGENT_SUPABASE_ANON_KEY";

#[derive(Clone, Debug)]
pub struct SyncConfig {
    pub url: String,
    pub anon_key: String,
}

impl SyncConfig {
    pub fn from_env() -> Option<SyncConfig> {
        let url = std::env::var(SUPABASE_URL_ENV).ok()?;
        let anon_key = std::env::var(SUPABASE_ANON_KEY_ENV).ok()?;
        if url.trim().is_empty() || anon_key.trim().is_empty() {
            return None;
        }
        Some(SyncConfig { url, anon_key })
    }
}

#[derive(Debug)]
pub enum SyncError {
    Config(String),
    Io(String),
    Network(String),
    RemoteStatus { code: u16 },
    Payload(String),
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncError::Config(m) => write!(f, "sync config: {m}"),
            SyncError::Io(m) => write!(f, "sync io: {m}"),
            SyncError::Network(m) => write!(f, "sync network: {m}"),
            SyncError::RemoteStatus { code } => write!(f, "sync remote rejected ({code})"),
            SyncError::Payload(m) => write!(f, "sync payload: {m}"),
        }
    }
}

pub trait SyncTransport: Send + Sync {
    fn post_json(&self, url: &str, headers: &[(&str, &str)], body: &[u8]) -> Result<(), SyncError>;
}

pub struct HttpTransport;

impl SyncTransport for HttpTransport {
    fn post_json(&self, url: &str, headers: &[(&str, &str)], body: &[u8]) -> Result<(), SyncError> {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(15))
            .build();
        let mut req = agent.post(url);
        for (name, value) in headers {
            req = req.set(name, value);
        }
        let resp = match req.send_bytes(body) {
            Ok(resp) => resp,
            Err(e) => return Err(SyncError::Network(e.to_string())),
        };
        let status = resp.status();
        if (200..400).contains(&status) {
            Ok(())
        } else {
            Err(SyncError::RemoteStatus { code: status })
        }
    }
}

pub struct AuditSyncClient {
    config: SyncConfig,
    transport: Box<dyn SyncTransport>,
}

impl AuditSyncClient {
    pub fn new(config: SyncConfig) -> Self {
        Self {
            config,
            transport: Box::new(HttpTransport),
        }
    }

    pub fn new_with(config: SyncConfig, transport: Box<dyn SyncTransport>) -> Self {
        Self { config, transport }
    }

    pub fn config(&self) -> &SyncConfig {
        &self.config
    }

    fn endpoint(&self) -> String {
        format!(
            "{}/rest/v1/audit_logs",
            self.config.url.trim_end_matches('/')
        )
    }

    fn headers<'a>(&'a self) -> Vec<(&'a str, String)> {
        vec![
            ("apikey", self.config.anon_key.clone()),
            ("Authorization", format!("Bearer {}", self.config.anon_key)),
            ("Content-Type", "application/json".to_string()),
            ("Prefer", "return=minimal".to_string()),
        ]
    }

    pub fn upload_report(&self, report: &Report) -> Result<(), SyncError> {
        // Audit metadata only: this payload is a POSTed *report row*, never the
        // recovered file contents (carving keeps those strictly on local disk).
        let body = json!({
            "report_id": report.report_id,
            "operation_id": report.operation_id,
            "operation_type": report.operation_type,
            "operator_id": report.operator_id,
            "target": report.target,
            "standard_id": report.standard_id,
            "standard_label": report.standard_label,
            "capacity_bytes": report.capacity_bytes,
            "start_time": report.start_time,
            "finish_time": report.finish_time,
            "verification_status": match report.verification.as_ref() {
                Some(v) => serde_json::to_value(v.status)
                    .ok()
                    .and_then(|x| x.as_str().map(|s| s.to_string()))
                    .unwrap_or_default(),
                None => String::new(),
            },
            "verification_detail": report
                .verification
                .as_ref()
                .map(|v| v.detail.clone())
                .unwrap_or_default(),
            "mismatched_sectors": report
                .verification
                .as_ref()
                .map(|v| v.mismatched_sectors)
                .unwrap_or(0),
            "skipped_sector_count": report.skipped_sector_count,
            "report_hash": report.report_hash,
            "signature_alg": report.signature_alg,
            "signature": report.signature,
        });
        let body = serde_json::to_vec(&body).map_err(|e| SyncError::Payload(e.to_string()))?;
        let url = self.endpoint();
        let owned_headers = self.headers();
        let headers: Vec<(&str, &str)> = owned_headers
            .iter()
            .map(|(k, v)| (*k, v.as_str()))
            .collect();
        self.transport.post_json(&url, &headers, &body)
    }

    /// Upload every report that is not yet marked as synced. Local operation is
    /// never blocked: a failing remote leaves reports marked unsynced so they are
    /// retried on the next sync pass. Returns (pushed_count, failed_report_ids).
    pub fn sync_unpushed(&self, db: &CaseDb) -> Result<(usize, Vec<String>), SyncError> {
        let pending = db
            .unsynced_report_ids()
            .map_err(|e| SyncError::Io(e.to_string()))?;
        let mut pushed = 0usize;
        let mut failed = Vec::new();
        for id in pending {
            let json = match db.report_json(&id) {
                Ok(j) => j,
                Err(_) => {
                    failed.push(id);
                    continue;
                }
            };
            let report: Report = match serde_json::from_str(&json) {
                Ok(r) => r,
                Err(_) => {
                    failed.push(id);
                    continue;
                }
            };
            match self.upload_report(&report) {
                Ok(()) => {
                    let _ = db.mark_synced(&id, "ok");
                    pushed += 1;
                }
                Err(_) => failed.push(id),
            }
        }
        Ok((pushed, failed))
    }
}
#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;
    use crate::modules::config::WipeStandard;
    use crate::modules::drive_eraser::{VerificationResult, VerificationStatus, WipeResult};
    use crate::modules::reporting::build_wipe_report;

    const TEST_KEY: &[u8] = b"phase-11-test-key-000000000000000000000000000";

    fn sample_report(report_id: &str) -> Report {
        let mut result = WipeResult {
            operation_id: "op-sync-1".into(),
            target: "image file C:\\test\\disk.img".into(),
            standard: WipeStandard::Nist800_88Clear,
            standard_label: "NIST Clear".into(),
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
        };
        result.operation_id = format!("{report_id}-op");
        let mut report = build_wipe_report(&result, TEST_KEY);
        report.report_id = report_id.to_string();
        crate::modules::reporting::sign_report(&mut report, TEST_KEY);
        report
    }

    struct MockTransport {
        tx: std::sync::Mutex<mpsc::Sender<Vec<u8>>>,
        status: u16,
    }

    impl SyncTransport for MockTransport {
        fn post_json(
            &self,
            _url: &str,
            _headers: &[(&str, &str)],
            body: &[u8],
        ) -> Result<(), SyncError> {
            let _ = self.tx.lock().unwrap().send(body.to_vec());
            if self.status == 201 {
                Ok(())
            } else {
                Err(SyncError::RemoteStatus { code: self.status })
            }
        }
    }

    fn config() -> SyncConfig {
        SyncConfig {
            url: "http://127.0.0.1:NOT_USED".to_string(),
            anon_key: "anon-key-test".to_string(),
        }
    }

    #[test]
    fn sync_uploads_audit_metadata_only() {
        let (tx, rx) = mpsc::channel();
        let report = sample_report("sync-upload-1");
        let client = AuditSyncClient::new_with(
            config(),
            Box::new(MockTransport {
                tx: std::sync::Mutex::new(tx),
                status: 201,
            }),
        );
        assert!(client.upload_report(&report).is_ok());
        let body = rx.recv().unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["report_id"], "sync-upload-1");
        assert_eq!(value["operation_type"], "secure_erase");
        assert_eq!(value["verification_status"], "passed");
        assert!(value["signature"]
            .as_str()
            .map(|s| !s.is_empty())
            .unwrap_or(false));
        assert!(!String::from_utf8_lossy(&body).contains("recovered-content"));
    }

    #[test]
    fn sync_unpushed_marks_only_successful() {
        let (tx, _rx) = mpsc::channel();
        let client = AuditSyncClient::new_with(
            config(),
            Box::new(MockTransport {
                tx: std::sync::Mutex::new(tx),
                status: 201,
            }),
        );
        let dir = std::env::temp_dir().join(format!("purgent-sync-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = CaseDb::open(&dir.join("case.sqlite3")).unwrap();
        for id in ["sync-a", "sync-b"] {
            let report = sample_report(id);
            let (json_path, pdf_path) =
                crate::modules::reporting::save_report(&report, &dir).unwrap();
            db.record_report(&report, &json_path, &pdf_path).unwrap();
        }
        let (pushed, failed) = client.sync_unpushed(&db).unwrap();
        assert_eq!(pushed, 2);
        assert!(failed.is_empty());
        assert_eq!(db.unsynced_report_ids().unwrap().len(), 0);
        assert_eq!(db.sync_counts().unwrap(), (2, 2));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sync_unpushed_keeps_failed_unsynced_for_retry() {
        let config = SyncConfig {
            url: "http://127.0.0.1:1".to_string(),
            anon_key: "anon-key-test".to_string(),
        };
        let client = AuditSyncClient::new(config);
        let dir = std::env::temp_dir().join(format!("purgent-sync-err-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = CaseDb::open(&dir.join("case.sqlite3")).unwrap();
        for id in ["sync-err-1", "sync-err-2"] {
            let report = sample_report(id);
            let (json_path, pdf_path) =
                crate::modules::reporting::save_report(&report, &dir).unwrap();
            db.record_report(&report, &json_path, &pdf_path).unwrap();
        }
        let (pushed, failed) = client.sync_unpushed(&db).unwrap();
        assert_eq!(pushed, 0, "remote unreachable must not erase pending state");
        assert_eq!(failed.len(), 2, "both ids remain queued for retry");
        assert_eq!(db.unsynced_report_ids().unwrap().len(), 2);
        assert_eq!(db.sync_counts().unwrap(), (0, 2));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn local_operation_completes_when_sync_unavailable() {
        let dir = std::env::temp_dir().join(format!("purgent-sync-off-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = CaseDb::open(&dir.join("case.sqlite3")).unwrap();
        let config = SyncConfig {
            url: "http://127.0.0.1:1".to_string(),
            anon_key: "anon-key-test".to_string(),
        };
        let client = AuditSyncClient::new(config);
        let report = sample_report("sync-offline-1");
        let (json_path, pdf_path) = crate::modules::reporting::save_report(&report, &dir).unwrap();
        db.record_report(&report, &json_path, &pdf_path).unwrap();
        assert_eq!(db.sync_counts().unwrap(), (0, 1));
        let (pushed, _) = client.sync_unpushed(&db).unwrap();
        assert_eq!(pushed, 0);
        let json = db.report_json(&report.report_id).unwrap();
        assert!(
            crate::modules::reporting::verify_report(json.as_bytes(), TEST_KEY),
            "report must be recorded and verifiable offline"
        );
        assert_eq!(db.unsynced_report_ids().unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
