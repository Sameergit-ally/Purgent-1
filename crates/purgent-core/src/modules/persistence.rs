use std::path::{Path, PathBuf};

use rusqlite::{params, OptionalExtension};

use super::reporting::Report;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersistError {
    Open(String),
    Query(String),
    NotFound(String),
    Io(String),
}

impl std::fmt::Display for PersistError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PersistError::Open(e) => write!(f, "cannot open case database: {e}"),
            PersistError::Query(e) => write!(f, "case database query failed: {e}"),
            PersistError::NotFound(id) => write!(f, "no recorded report with id {id}"),
            PersistError::Io(e) => write!(f, "case database io: {e}"),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReportRow {
    pub report_id: String,
    pub operation_id: String,
    pub operation_type: String,
    pub operator_id: String,
    pub target: String,
    pub standard_id: String,
    pub standard_label: String,
    pub capacity_bytes: u64,
    pub start_time: String,
    pub finish_time: String,
    pub verification_status: String,
    pub mismatched_sectors: u64,
    pub skipped_sector_count: u64,
    pub report_hash: String,
    pub signature_alg: String,
    pub signature: String,
    pub json_path: String,
    pub pdf_path: String,
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS report_records (
    report_id TEXT PRIMARY KEY,
    operation_id TEXT NOT NULL,
    operation_type TEXT NOT NULL,
    operator_id TEXT NOT NULL,
    target TEXT NOT NULL,
    standard_id TEXT NOT NULL,
    standard_label TEXT NOT NULL,
    capacity_bytes INTEGER NOT NULL,
    start_time TEXT NOT NULL,
    finish_time TEXT NOT NULL,
    verification_status TEXT NOT NULL,
    mismatched_sectors INTEGER NOT NULL DEFAULT 0,
    skipped_sector_count INTEGER NOT NULL DEFAULT 0,
    report_hash TEXT NOT NULL,
    signature_alg TEXT NOT NULL,
    signature TEXT NOT NULL,
    json_path TEXT NOT NULL,
    pdf_path TEXT NOT NULL,
    json_content TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_report_records_started ON report_records(start_time DESC);
CREATE INDEX IF NOT EXISTS idx_report_records_operation ON report_records(operation_type);
CREATE TABLE IF NOT EXISTS sync_state (
    report_id TEXT PRIMARY KEY REFERENCES report_records(report_id) ON DELETE CASCADE,
    synced_at TEXT NOT NULL,
    remote_status TEXT NOT NULL
);
";

pub struct CaseDb {
    conn: rusqlite::Connection,
}

impl CaseDb {
    pub fn open(path: &Path) -> Result<CaseDb, PersistError> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| PersistError::Io(e.to_string()))?;
            }
        }
        let conn =
            rusqlite::Connection::open(path).map_err(|e| PersistError::Open(e.to_string()))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .map_err(|e| PersistError::Query(e.to_string()))?;
        conn.execute_batch(SCHEMA)
            .map_err(|e| PersistError::Query(e.to_string()))?;
        Ok(CaseDb { conn })
    }

    pub fn record_report(
        &self,
        report: &Report,
        json_path: &Path,
        pdf_path: &Path,
    ) -> Result<(), PersistError> {
        let json_content =
            serde_json::to_string(report).map_err(|e| PersistError::Query(e.to_string()))?;
        let verification_status = report
            .verification
            .as_ref()
            .and_then(|v| serde_json::to_value(v.status).ok())
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_default();
        let mismatched_sectors = report
            .verification
            .as_ref()
            .map(|v| v.mismatched_sectors as i64)
            .unwrap_or(0);
        self.conn
            .execute(
                "INSERT INTO report_records (
                    report_id, operation_id, operation_type, operator_id, target,
                    standard_id, standard_label, capacity_bytes, start_time, finish_time,
                    verification_status, mismatched_sectors, skipped_sector_count,
                    report_hash, signature_alg, signature, json_path, pdf_path, json_content
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)
                ON CONFLICT(report_id) DO UPDATE SET
                    operation_id = excluded.operation_id,
                    operation_type = excluded.operation_type,
                    operator_id = excluded.operator_id,
                    target = excluded.target,
                    standard_id = excluded.standard_id,
                    standard_label = excluded.standard_label,
                    capacity_bytes = excluded.capacity_bytes,
                    start_time = excluded.start_time,
                    finish_time = excluded.finish_time,
                    verification_status = excluded.verification_status,
                    mismatched_sectors = excluded.mismatched_sectors,
                    skipped_sector_count = excluded.skipped_sector_count,
                    report_hash = excluded.report_hash,
                    signature_alg = excluded.signature_alg,
                    signature = excluded.signature,
                    json_path = excluded.json_path,
                    pdf_path = excluded.pdf_path,
                    json_content = excluded.json_content",
                params![
                    report.report_id,
                    report.operation_id,
                    report.operation_type,
                    report.operator_id,
                    report.target,
                    report.standard_id,
                    report.standard_label,
                    report.capacity_bytes as i64,
                    report.start_time,
                    report.finish_time,
                    verification_status,
                    mismatched_sectors,
                    report.skipped_sector_count as i64,
                    report.report_hash,
                    report.signature_alg,
                    report.signature,
                    json_path.display().to_string(),
                    pdf_path.display().to_string(),
                    json_content,
                ],
            )
            .map_err(|e| PersistError::Query(e.to_string()))?;
        Ok(())
    }

    pub fn has_report(&self, report_id: &str) -> Result<bool, PersistError> {
        let found: Option<i64> = self
            .conn
            .query_row(
                "SELECT 1 FROM report_records WHERE report_id = ?1",
                params![report_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| PersistError::Query(e.to_string()))?;
        Ok(found.is_some())
    }

    pub fn all_report_meta(&self) -> Result<Vec<ReportRow>, PersistError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT report_id, operation_id, operation_type, operator_id, target,
                        standard_id, standard_label, capacity_bytes, start_time, finish_time,
                        verification_status, mismatched_sectors, skipped_sector_count,
                        report_hash, signature_alg, signature, json_path, pdf_path
                 FROM report_records ORDER BY start_time DESC",
            )
            .map_err(|e| PersistError::Query(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(ReportRow {
                    report_id: row.get(0)?,
                    operation_id: row.get(1)?,
                    operation_type: row.get(2)?,
                    operator_id: row.get(3)?,
                    target: row.get(4)?,
                    standard_id: row.get(5)?,
                    standard_label: row.get(6)?,
                    capacity_bytes: row.get::<_, i64>(7)? as u64,
                    start_time: row.get(8)?,
                    finish_time: row.get(9)?,
                    verification_status: row.get(10)?,
                    mismatched_sectors: row.get::<_, i64>(11)? as u64,
                    skipped_sector_count: row.get::<_, i64>(12)? as u64,
                    report_hash: row.get(13)?,
                    signature_alg: row.get(14)?,
                    signature: row.get(15)?,
                    json_path: row.get(16)?,
                    pdf_path: row.get(17)?,
                })
            })
            .map_err(|e| PersistError::Query(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| PersistError::Query(e.to_string()))?;
        Ok(rows)
    }

    pub fn report_json(&self, report_id: &str) -> Result<String, PersistError> {
        let content: Option<String> = self
            .conn
            .query_row(
                "SELECT json_content FROM report_records WHERE report_id = ?1",
                params![report_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| PersistError::Query(e.to_string()))?;
        content.ok_or_else(|| PersistError::NotFound(report_id.to_string()))
    }

    pub fn report_paths(&self, report_id: &str) -> Result<(PathBuf, PathBuf), PersistError> {
        let paths: Option<(String, String)> = self
            .conn
            .query_row(
                "SELECT json_path, pdf_path FROM report_records WHERE report_id = ?1",
                params![report_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|e| PersistError::Query(e.to_string()))?;
        match paths {
            Some((json_path, pdf_path)) => Ok((PathBuf::from(json_path), PathBuf::from(pdf_path))),
            None => Err(PersistError::NotFound(report_id.to_string())),
        }
    }

    pub fn mark_synced(&self, report_id: &str, remote_status: &str) -> Result<(), PersistError> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO sync_state (report_id, synced_at, remote_status)
                 VALUES (?1, ?2, ?3)",
                params![
                    report_id,
                    crate::modules::log::now_utc_rfc3339(),
                    remote_status
                ],
            )
            .map_err(|e| PersistError::Query(e.to_string()))?;
        Ok(())
    }

    pub fn unsynced_report_ids(&self) -> Result<Vec<String>, PersistError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT r.report_id FROM report_records r
                 LEFT JOIN sync_state s ON s.report_id = r.report_id
                 WHERE s.report_id IS NULL
                 ORDER BY r.start_time ASC",
            )
            .map_err(|e| PersistError::Query(e.to_string()))?;
        let ids = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| PersistError::Query(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| PersistError::Query(e.to_string()))?;
        Ok(ids)
    }

    pub fn sync_counts(&self) -> Result<(u64, u64), PersistError> {
        let total: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM report_records", [], |row| row.get(0))
            .map_err(|e| PersistError::Query(e.to_string()))?;
        let synced: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM sync_state", [], |row| row.get(0))
            .map_err(|e| PersistError::Query(e.to_string()))?;
        Ok((synced as u64, total as u64))
    }

    pub fn import_directory(&self, dir: &Path) -> Result<u64, PersistError> {
        let mut imported = 0u64;
        if !dir.is_dir() {
            return Ok(imported);
        }
        let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
            .map_err(|e| PersistError::Io(e.to_string()))?
            .filter_map(|de| de.ok())
            .map(|de| de.path())
            .filter(|p| {
                p.extension().map(|e| e == "json").unwrap_or(false)
                    && p.file_name()
                        .map(|n| n.to_string_lossy().starts_with("report-"))
                        .unwrap_or(false)
            })
            .collect();
        entries.sort();
        for path in entries {
            let json_content =
                std::fs::read_to_string(&path).map_err(|e| PersistError::Io(e.to_string()))?;
            let report: Report = serde_json::from_str(&json_content)
                .map_err(|e| PersistError::Query(e.to_string()))?;
            if self.has_report(&report.report_id)? {
                continue;
            }
            let pdf_path = path.with_extension("pdf");
            self.record_report(&report, &path, &pdf_path)?;
            imported += 1;
        }
        Ok(imported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::config::WipeStandard;
    use crate::modules::drive_eraser::{WipeRequest, WipeTarget};
    use crate::modules::reporting;

    const TEST_KEY: &[u8] = b"persistence-test-key-00000000000000000000000000";

    fn wipe_and_save(dir: &Path) -> (Report, PathBuf, PathBuf) {
        let image = dir.join("wipe.img");
        std::fs::write(&image, vec![0x55u8; 512 * 8]).unwrap();
        let report_dir = dir.join("reports");
        let request = WipeRequest {
            operator_id: "opr-persist".into(),
            target: WipeTarget::ImageFile(image.clone()),
            standard: WipeStandard::Nist800_88Clear,
            operator_confirmed_target: format!("image file {}", image.display()),
            block_size: None,
            fallback_acknowledged: false,
        };
        let result = crate::modules::drive_eraser::wipe(request).unwrap();
        let report = reporting::build_wipe_report(&result, TEST_KEY);
        let (json_path, pdf_path) = reporting::save_report(&report, &report_dir).unwrap();
        (report, json_path, pdf_path)
    }

    #[test]
    fn report_roundtrip_survives_db_restart_fully_offline() {
        let dir = std::env::temp_dir().join(format!("purgent-persist-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let (report, json_path, pdf_path) = wipe_and_save(&dir);
        let db_path = dir.join("case.sqlite3");

        {
            let db = CaseDb::open(&db_path).unwrap();
            db.record_report(&report, &json_path, &pdf_path).unwrap();
            assert_eq!(db.all_report_meta().unwrap().len(), 1);
        }

        {
            let db = CaseDb::open(&db_path).unwrap();
            let rows = db.all_report_meta().unwrap();
            assert_eq!(rows.len(), 1, "prior session data must survive relaunch");
            let row = &rows[0];
            assert_eq!(row.report_id, report.report_id);
            assert_eq!(row.operation_type, "secure_erase");
            assert_eq!(row.operator_id, "opr-persist");
            assert_eq!(row.verification_status, "passed");
            assert_eq!(row.signature_alg, "HMAC-SHA256");

            let stored = db.report_json(&report.report_id).unwrap();
            let parsed: Report = serde_json::from_str(&stored).unwrap();
            assert_eq!(parsed.report_hash, report.report_hash);
            assert!(
                reporting::verify_report(stored.as_bytes(), TEST_KEY),
                "stored report must verify offline with the same key"
            );
            let (stored_json, stored_pdf) = db.report_paths(&report.report_id).unwrap();
            assert_eq!(stored_json, json_path);
            assert!(stored_pdf.extension().map(|e| e == "pdf").unwrap_or(false));
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn fresh_db_is_empty_and_question_marks_are_queryable() {
        let dir =
            std::env::temp_dir().join(format!("purgent-persist-fresh-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("case.sqlite3");
        let db = CaseDb::open(&db_path).unwrap();
        assert!(db.all_report_meta().unwrap().is_empty());
        assert!(!db.has_report("nope").unwrap());
        assert!(matches!(
            db.report_json("nope"),
            Err(PersistError::NotFound(_))
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn import_directory_backfills_legacy_reports() {
        let dir =
            std::env::temp_dir().join(format!("purgent-persist-import-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let (report, json_path, _pdf_path) = wipe_and_save(&dir);
        let reports_dir = json_path.parent().unwrap().to_path_buf();

        let db_path = dir.join("case.sqlite3");
        let db = CaseDb::open(&db_path).unwrap();
        let imported = db.import_directory(&reports_dir).unwrap();
        assert_eq!(imported, 1);

        let second = db.import_directory(&reports_dir).unwrap();
        assert_eq!(second, 0, "re-import must not duplicate");
        assert_eq!(db.all_report_meta().unwrap().len(), 1);

        let db_path2 = dir.join("case2.sqlite3");
        let db2 = CaseDb::open(&db_path2).unwrap();
        assert_eq!(db2.import_directory(&reports_dir).unwrap(), 1);
        let stored = db2.report_json(&report.report_id).unwrap();
        assert!(reporting::verify_report(stored.as_bytes(), TEST_KEY));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sync_state_tracks_pushed_reports_across_restart() {
        let dir =
            std::env::temp_dir().join(format!("purgent-persist-sync-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let (report, json_path, pdf_path) = wipe_and_save(&dir);
        let (other, other_json, other_pdf) = wipe_and_save(&dir);
        let db_path = dir.join("case.sqlite3");

        {
            let db = CaseDb::open(&db_path).unwrap();
            db.record_report(&report, &json_path, &pdf_path).unwrap();
            db.record_report(&other, &other_json, &other_pdf).unwrap();
            assert_eq!(db.unsynced_report_ids().unwrap().len(), 2);
            db.mark_synced(&report.report_id, "201").unwrap();
            let ids = db.unsynced_report_ids().unwrap();
            assert_eq!(ids.len(), 1);
            assert_eq!(ids[0], other.report_id);
            assert_eq!(db.sync_counts().unwrap(), (1, 2));
        }

        {
            let db = CaseDb::open(&db_path).unwrap();
            let ids = db.unsynced_report_ids().unwrap();
            assert_eq!(ids.len(), 1, "sync state must survive relaunch");
            assert_eq!(ids[0], other.report_id);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
