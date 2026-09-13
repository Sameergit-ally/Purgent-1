#[derive(Debug, Clone, serde::Serialize)]
pub struct ProgressUpdate {
    pub operation_id: String,
    pub operation_type: String,
    pub phase: String,
    pub bytes_done: u64,
    pub total_bytes: u64,
    pub message: String,
}

impl ProgressUpdate {
    pub fn fraction(&self) -> f64 {
        if self.total_bytes == 0 {
            0.0
        } else {
            (self.bytes_done as f64 / self.total_bytes as f64).clamp(0.0, 1.0)
        }
    }
}

pub type ProgressFn<'a> = &'a mut dyn FnMut(ProgressUpdate);
