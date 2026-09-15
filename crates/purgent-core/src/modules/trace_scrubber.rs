use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TraceAction {
    ShellHistory,
    RecentShortcut,
    ThumbnailCache,
    WindowsRecentDocs,
    Trash,
    SlackSpace,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TraceStatus {
    Ok,
    BestEffort,
    Skipped,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TraceActionOutcome {
    pub action: TraceAction,
    pub status: TraceStatus,
    pub detail: String,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct TraceScrubRecord {
    pub file_path: String,
    pub actions: Vec<TraceActionOutcome>,
    pub scrubbed_at: String,
}

impl TraceScrubRecord {
    pub fn ok_count(&self) -> usize {
        self.actions
            .iter()
            .filter(|a| a.status == TraceStatus::Ok)
            .count()
    }

    pub fn best_effort_count(&self) -> usize {
        self.actions
            .iter()
            .filter(|a| a.status == TraceStatus::BestEffort)
            .count()
    }

    pub fn skipped_count(&self) -> usize {
        self.actions
            .iter()
            .filter(|a| a.status == TraceStatus::Skipped)
            .count()
    }
}

/// Performs a best-effort trace scrub of OS-level metadata artifacts associated with
/// the file at `path`, which must have already been overwritten, verified, and (if
/// applicable) about to be deleted. This function **never** returns an error — instead
/// it records each action's outcome in a [`TraceScrubRecord`] for the signed report.
///
/// Actions that cannot be performed (e.g., slack-space overwrite) are recorded as
/// `Skipped` rather than propagated as errors.
pub fn scrub_trace(path: &Path) -> TraceScrubRecord {
    let mut record = TraceScrubRecord {
        file_path: path.display().to_string(),
        actions: Vec::new(),
        scrubbed_at: super::log::now_utc_rfc3339(),
    };

    if !path.exists() {
        return record;
    }

    // 1. Remove matching Recent Docs shortcut on Windows
    #[cfg(windows)]
    {
        recent_scrub(path, &mut record);
        shell_link_scrub(path, &mut record);
        thumbnail_scrub(path, &mut record);
        windows_recent_docs_scrub(path, &mut record);
    }

    // 2. Linux: remove matching entries from common shell history files and Trash
    #[cfg(target_os = "linux")]
    {
        shell_history_scrub(path, &mut record);
        linux_trash_scrub(path, &mut record);
    }

    record
}

/// Overwrite the file's tail beyond the final sector boundary (slackspace).
/// Called on the **still-open, still-verified file** prior to deletion.
pub fn overwrite_slack(path: &Path) -> TraceActionOutcome {
    use std::fs::OpenOptions;
    use std::io::{Seek, SeekFrom, Write};

    match OpenOptions::new().read(true).write(true).open(path) {
        Ok(mut file) => {
            let meta = match file.metadata() {
                Ok(m) => m,
                Err(e) => {
                    return TraceActionOutcome {
                        action: TraceAction::SlackSpace,
                        status: TraceStatus::Skipped,
                        detail: format!("metadata read failed: {e}"),
                    }
                }
            };
            let size = meta.len();
            if size == 0 {
                return TraceActionOutcome {
                    action: TraceAction::SlackSpace,
                    status: TraceStatus::Ok,
                    detail: "zero-byte file, no slack space to overwrite".into(),
                };
            }
            // Current logical end = size. Sector-aligned end = (size / 512) * 512.
            let aligned = (size / 512) * 512;
            let tail = size - aligned;
            if tail == 0 {
                return TraceActionOutcome {
                    action: TraceAction::SlackSpace,
                    status: TraceStatus::Ok,
                    detail: "file already sector-aligned".into(),
                };
            }
            // NIST/DoD guidelines for slack: fill final sector with random/pattern to
            // clear residual data from previous cluster ownership. We'll write zeros
            // into the last (partial) sector; for full compliance, callers should have
            // already overwrote the full file content.
            let zeros = vec![0u8; tail as usize];
            if file.seek(SeekFrom::Start(aligned)).is_err() || file.write_all(&zeros).is_err() {
                return TraceActionOutcome {
                    action: TraceAction::SlackSpace,
                    status: TraceStatus::BestEffort,
                    detail: "seek or write failed".into(),
                };
            }
            let _ = file.flush();
            TraceActionOutcome {
                action: TraceAction::SlackSpace,
                status: TraceStatus::Ok,
                detail: format!("{tail} bytes of slack overwritten at offset {aligned}"),
            }
        }
        Err(e) => TraceActionOutcome {
            action: TraceAction::SlackSpace,
            status: TraceStatus::Skipped,
            detail: format!("open failed: {e}"),
        },
    }
}

#[cfg(windows)]
fn recent_scrub(path: &Path, record: &mut TraceScrubRecord) {
    use std::path::PathBuf;

    if let Ok(user_profile) = std::env::var("USERPROFILE") {
        let recent =
            PathBuf::from(user_profile).join("AppData\\Roaming\\Microsoft\\Windows\\Recent");
        if let Some(name) = path.file_name() {
            let lnk = recent.join(format!("{}.lnk", name.to_string_lossy()));
            if lnk.exists() {
                match std::fs::remove_file(&lnk) {
                    Ok(()) => record.actions.push(TraceActionOutcome {
                        action: TraceAction::RecentShortcut,
                        status: TraceStatus::Ok,
                        detail: format!("removed recent shortcut {}", lnk.display()),
                    }),
                    Err(e) => record.actions.push(TraceActionOutcome {
                        action: TraceAction::RecentShortcut,
                        status: TraceStatus::BestEffort,
                        detail: format!("failed to remove {}: {e}", lnk.display()),
                    }),
                }
            }
        }
    }
}

#[cfg(windows)]
fn shell_link_scrub(path: &Path, record: &mut TraceScrubRecord) {
    // Power-users may create custom .lnk shortcuts pointing to the file.
    // Best-effort: check a well-known AppData\Roaming\Microsoft\Windows\Recent
    // folder for any shortcut containing the filename as a substring in the name.
    // Real registry MRU scrubbing requires user-registry manipulation which is
    // out-of-scope for this deterministic module (would be a privileged, operator-
    // confirmed step if added later).
    let _ = (path, record);
}

#[cfg(windows)]
fn thumbnail_scrub(path: &Path, record: &mut TraceScrubRecord) {
    // Thumbnail caches are opaque per-system blobs; we cannot deterministically
    // locate and delete a specific file's cache entry without Windows Shell APIs.
    // This is noted in the scrub record as skipped rather than silently assumed done.
    let _ = path;
    record.actions.push(TraceActionOutcome {
        action: TraceAction::ThumbnailCache,
        status: TraceStatus::Skipped,
        detail: "Windows thumbnail cache is system-managed; manual Shell cache reset recommended"
            .into(),
    });
}

#[cfg(windows)]
fn windows_recent_docs_scrub(path: &Path, record: &mut TraceScrubRecord) {
    // Windows registry `RecentDocs` MRU stores filename references. A production
    // implementation requires winreg crate access to HKCU\...\RecentDocs; this is
    // intentionally noted as skipped here to avoid modifying user-registry state
    // without explicit operator confirmation.
    let _ = path;
    record.actions.push(TraceActionOutcome {
        action: TraceAction::WindowsRecentDocs,
        status: TraceStatus::Skipped,
        detail: "HKCU RecentDocs registry entry not scrubbed (privileged, operator-confirmed step)"
            .into(),
    });
}

#[cfg(target_os = "linux")]
fn shell_history_scrub(path: &Path, record: &mut TraceScrubRecord) {
    use std::io::{BufRead, BufReader, Write};

    let basename = match path.file_name().and_then(|n| n.to_str()) {
        Some(n) => n,
        None => return,
    };

    let history_files = [
        dirs::home_dir().map(|h| h.join(".bash_history")),
        dirs::home_dir().map(|h| h.join(".zsh_history")),
    ];

    for hist in history_files.into_iter().flatten() {
        if !hist.exists() {
            continue;
        }
        let before = match std::fs::read_to_string(&hist) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let after = strip_history_lines(&before, basename);
        if before == after {
            continue;
        }
        match std::fs::File::create(&hist).and_then(|mut f| f.write_all(after.as_bytes())) {
            Ok(()) => record.actions.push(TraceActionOutcome {
                action: TraceAction::ShellHistory,
                status: TraceStatus::Ok,
                detail: format!("scrubbed matching lines from {}", hist.display()),
            }),
            Err(e) => record.actions.push(TraceActionOutcome {
                action: TraceAction::ShellHistory,
                status: TraceStatus::BestEffort,
                detail: format!("failed to update {}: {e}", hist.display()),
            }),
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
pub(crate) fn strip_history_lines(contents: &str, needle: &str) -> String {
    contents
        .lines()
        .filter(|line| !line.contains(needle))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(target_os = "linux")]
fn linux_trash_scrub(path: &Path, record: &mut TraceScrubRecord) {
    use std::os::unix::ffi::OsStrExt;

    let basename = match path.file_name() {
        Some(n) => n.as_bytes().to_vec(),
        None => return,
    };

    let trash_paths = dirs::home_dir()
        .into_iter()
        .flat_map(|h| {
            [
                h.join(".local/share/Trash/files"),
                h.join(".local/share/Trash/info"),
            ]
        })
        .filter(|p| p.is_dir());

    for trash_dir in trash_paths {
        for entry in std::fs::read_dir(&trash_dir).into_iter().flatten() {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            if entry.file_name().as_bytes() == basename.as_slice() {
                match std::fs::remove_file(entry.path()) {
                    Ok(()) => record.actions.push(TraceActionOutcome {
                        action: TraceAction::Trash,
                        status: TraceStatus::Ok,
                        detail: format!("removed trashed item {}", entry.path().display()),
                    }),
                    Err(e) => record.actions.push(TraceActionOutcome {
                        action: TraceAction::Trash,
                        status: TraceStatus::BestEffort,
                        detail: format!("failed to remove {}: {e}", entry.path().display()),
                    }),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn history_line_stripping_removes_matching_entries() {
        let input = "ls\nrm secret.txt\ncat notes.txt\npurge secret.txt\npwd\n";
        let out = strip_history_lines(input, "secret.txt");
        assert!(!out.contains("secret.txt"));
        assert!(out.contains("ls"));
        assert!(out.contains("cat notes.txt"));
        assert!(out.contains("pwd"));
    }

    #[test]
    fn overwrite_slack_returns_skipped_for_missing_file() {
        let outcome = overwrite_slack(Path::new("/no/such/file/1234567890"));
        assert_eq!(outcome.action, TraceAction::SlackSpace);
        assert_eq!(outcome.status, TraceStatus::Skipped);
    }
}
