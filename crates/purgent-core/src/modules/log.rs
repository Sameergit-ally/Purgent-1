use std::io::Write;
use std::sync::Mutex;

use chrono::Utc;

static LOCK: Mutex<()> = Mutex::new(());

pub fn now_utc_rfc3339() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub fn log(state: &str, details: &str) {
    let _guard = LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    if details.is_empty() {
        let _ = writeln!(out, "[purgent] [{}] {}", now_utc_rfc3339(), state);
    } else {
        let _ = writeln!(
            out,
            "[purgent] [{}] {} | {}",
            now_utc_rfc3339(),
            state,
            details
        );
    }
}
