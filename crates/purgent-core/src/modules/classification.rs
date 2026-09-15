use crate::modules::recovery::RecoveredFile;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileCategory {
    Image,
    Video,
    Audio,
    Document,
    Archive,
    Executable,
    Other,
}

impl FileCategory {
    pub fn label(self) -> &'static str {
        match self {
            FileCategory::Image => "Image",
            FileCategory::Video => "Video",
            FileCategory::Audio => "Audio",
            FileCategory::Document => "Document",
            FileCategory::Archive => "Archive",
            FileCategory::Executable => "Executable",
            FileCategory::Other => "Other",
        }
    }
}

pub fn classify_recovered(recovered: &RecoveredFile) -> FileCategory {
    classify(recovered.signature.as_str(), recovered.mime.as_str())
}

pub fn classify(sig_id: &str, mime: &str) -> FileCategory {
    match sig_id {
        "jpeg" | "png" | "gif" | "bmp" => FileCategory::Image,
        "mp4" | "mov" | "avi" | "mkv" => FileCategory::Video,
        "wav" | "mp3" | "flac" => FileCategory::Audio,
        "pdf" | "docx" | "xlsx" | "pptx" | "txt" => FileCategory::Document,
        "zip" | "7z" | "rar" => FileCategory::Archive,
        "exe" | "dll" => FileCategory::Executable,
        _ => {
            if mime.starts_with("image/") {
                FileCategory::Image
            } else if mime.starts_with("video/") {
                FileCategory::Video
            } else if mime.starts_with("audio/") {
                FileCategory::Audio
            } else if mime.starts_with("application/") {
                if mime.contains("zip") || mime.contains("compressed") || mime.contains("archive") {
                    FileCategory::Archive
                } else if mime.contains("pdf")
                    || mime.contains("word")
                    || mime.contains("sheet")
                    || mime.contains("presentation")
                {
                    FileCategory::Document
                } else if mime.contains("executable") || mime.contains("x-msdownload") {
                    FileCategory::Executable
                } else {
                    FileCategory::Other
                }
            } else {
                FileCategory::Other
            }
        }
    }
}

pub fn aggregate_categories(files: &[RecoveredFile]) -> Vec<(FileCategory, usize)> {
    use std::collections::HashMap;
    let mut counts: HashMap<FileCategory, usize> = HashMap::new();
    for f in files {
        let cat = classify_recovered(f);
        *counts.entry(cat).or_insert(0) += 1;
    }
    let mut out: Vec<(FileCategory, usize)> = counts.into_iter().collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.label().cmp(b.0.label())));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jpeg_classifies_as_image() {
        assert_eq!(classify("jpeg", "image/jpeg"), FileCategory::Image);
    }

    #[test]
    fn docx_classifies_as_document() {
        assert_eq!(
            classify(
                "docx",
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            ),
            FileCategory::Document
        );
    }

    #[test]
    fn mp4_classifies_as_video() {
        assert_eq!(classify("mp4", "video/mp4"), FileCategory::Video);
    }

    #[test]
    fn fallback_mime_prefix_rules() {
        assert_eq!(classify("unknown", "image/bmp"), FileCategory::Image);
        assert_eq!(
            classify("unknown", "application/pdf"),
            FileCategory::Document
        );
        assert_eq!(
            classify("unknown", "application/x-executable"),
            FileCategory::Executable
        );
    }
}
