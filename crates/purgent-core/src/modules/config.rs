#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PassPattern {
    Zeros,
    Ones,
    Fixed(u8),
    SeededRandom,
}

impl PassPattern {
    pub fn byte(self) -> Option<u8> {
        match self {
            PassPattern::Zeros => Some(0x00),
            PassPattern::Ones => Some(0xFF),
            PassPattern::Fixed(v) => Some(v),
            PassPattern::SeededRandom => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WipeStandard {
    Nist800_88Clear,
    Nist800_88Purge,
    Dod522022M,
    Ieee2883Purge,
    Iso27037Capture,
    AtaSecureErase,
    NvmeSanitize,
}

impl WipeStandard {
    pub fn label(self) -> &'static str {
        match self {
            WipeStandard::Nist800_88Clear => "NIST SP 800-88 Rev.1 Clear",
            WipeStandard::Nist800_88Purge => "NIST SP 800-88 Rev.1 Purge",
            WipeStandard::Dod522022M => "DoD 5220.22-M",
            WipeStandard::Ieee2883Purge => "IEEE 2883-2022 Purge",
            WipeStandard::Iso27037Capture => "ISO/IEC 27037 Evidence Handling",
            WipeStandard::AtaSecureErase => "ATA Secure Erase (in-band)",
            WipeStandard::NvmeSanitize => "NVMe Sanitize - Crypto Erase (in-band)",
        }
    }

    pub fn is_hardware_method(self) -> bool {
        matches!(
            self,
            WipeStandard::AtaSecureErase | WipeStandard::NvmeSanitize
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WipeMethod {
    Overwrite,
    AtaSecureErase,
    NvmeSanitize,
}

impl WipeMethod {
    pub fn label(self) -> &'static str {
        match self {
            WipeMethod::Overwrite => "overwrite",
            WipeMethod::AtaSecureErase => "ata_secure_erase",
            WipeMethod::NvmeSanitize => "nvme_sanitize",
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WipeSpec {
    pub standard: WipeStandard,
    pub method: WipeMethod,
    pub passes: Vec<PassPattern>,
    pub verify_after: bool,
    pub capture_evidence_before: bool,
    pub seed: u64,
}

pub const RANDOM_SEED: u64 = 0x9E37_79B9_7F4A_7C15;

pub fn supported_wipe_standards() -> &'static [WipeSpec] {
    use std::sync::OnceLock;
    static STANDARDS: OnceLock<Vec<WipeSpec>> = OnceLock::new();
    STANDARDS
        .get_or_init(|| {
            vec![
                WipeSpec {
                    standard: WipeStandard::Nist800_88Clear,
                    method: WipeMethod::Overwrite,
                    passes: vec![PassPattern::Zeros],
                    verify_after: true,
                    capture_evidence_before: false,
                    seed: RANDOM_SEED,
                },
                WipeSpec {
                    standard: WipeStandard::Nist800_88Purge,
                    method: WipeMethod::Overwrite,
                    passes: vec![PassPattern::Zeros],
                    verify_after: true,
                    capture_evidence_before: false,
                    seed: RANDOM_SEED,
                },
                WipeSpec {
                    standard: WipeStandard::Dod522022M,
                    method: WipeMethod::Overwrite,
                    passes: vec![
                        PassPattern::Zeros,
                        PassPattern::Ones,
                        PassPattern::SeededRandom,
                    ],
                    verify_after: true,
                    capture_evidence_before: false,
                    seed: RANDOM_SEED,
                },
                WipeSpec {
                    standard: WipeStandard::Ieee2883Purge,
                    method: WipeMethod::Overwrite,
                    passes: vec![PassPattern::SeededRandom],
                    verify_after: true,
                    capture_evidence_before: false,
                    seed: RANDOM_SEED,
                },
                WipeSpec {
                    standard: WipeStandard::Iso27037Capture,
                    method: WipeMethod::Overwrite,
                    passes: vec![PassPattern::SeededRandom],
                    verify_after: true,
                    capture_evidence_before: true,
                    seed: RANDOM_SEED,
                },
                WipeSpec {
                    standard: WipeStandard::AtaSecureErase,
                    method: WipeMethod::AtaSecureErase,
                    passes: vec![PassPattern::SeededRandom],
                    verify_after: true,
                    capture_evidence_before: false,
                    seed: RANDOM_SEED,
                },
                WipeSpec {
                    standard: WipeStandard::NvmeSanitize,
                    method: WipeMethod::NvmeSanitize,
                    passes: vec![PassPattern::SeededRandom],
                    verify_after: true,
                    capture_evidence_before: false,
                    seed: RANDOM_SEED,
                },
            ]
        })
        .as_slice()
}

pub fn wipe_spec(standard: WipeStandard) -> WipeSpec {
    supported_wipe_standards()
        .iter()
        .find(|s| s.standard == standard)
        .cloned()
        .expect("supported wipe standard is present")
}

/// Overwrite profile used when a hardware method (ATA Secure Erase / NVMe Sanitize)
/// is requested but unavailable and the operator has explicitly acknowledged the
/// fallback. Per RULES this fallback is never silent.
pub fn fallback_wipe_spec(standard: WipeStandard) -> WipeSpec {
    if standard.is_hardware_method() {
        WipeSpec {
            standard,
            method: WipeMethod::Overwrite,
            passes: vec![PassPattern::SeededRandom],
            verify_after: true,
            capture_evidence_before: false,
            seed: RANDOM_SEED,
        }
    } else {
        wipe_spec(standard)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureEnd {
    Terminator(&'static [u8]),
    ByteTerminator(u8),
    FixedSize { size_offset: usize },
    ZipEocd,
    Mp4Moov,
    TrailingEof,
}

#[derive(Debug, Clone, Copy)]
pub struct FileSignature {
    pub id: &'static str,
    pub extension: &'static str,
    pub mime: &'static str,
    pub magic: &'static [u8],
    pub magic_offset: usize,
    pub end: SignatureEnd,
    pub max_carve_size: u64,
}

pub fn signature_database() -> &'static [FileSignature] {
    &[
        FileSignature {
            id: "jpeg",
            extension: "jpg",
            mime: "image/jpeg",
            magic: b"\xFF\xD8\xFF",
            magic_offset: 0,
            end: SignatureEnd::Terminator(b"\xFF\xD9"),
            max_carve_size: 64 * 1024 * 1024,
        },
        FileSignature {
            id: "png",
            extension: "png",
            mime: "image/png",
            magic: b"\x89PNG\r\n\x1A\n",
            magic_offset: 0,
            end: SignatureEnd::Terminator(b"\x49\x45\x4E\x44\xAE\x42\x60\x82"),
            max_carve_size: 128 * 1024 * 1024,
        },
        FileSignature {
            id: "gif",
            extension: "gif",
            mime: "image/gif",
            magic: b"GIF8",
            magic_offset: 0,
            end: SignatureEnd::ByteTerminator(0x3B),
            max_carve_size: 64 * 1024 * 1024,
        },
        FileSignature {
            id: "bmp",
            extension: "bmp",
            mime: "image/bmp",
            magic: b"BM",
            magic_offset: 0,
            end: SignatureEnd::FixedSize { size_offset: 2 },
            max_carve_size: 512 * 1024 * 1024,
        },
        FileSignature {
            id: "pdf",
            extension: "pdf",
            mime: "application/pdf",
            magic: b"%PDF-",
            magic_offset: 0,
            end: SignatureEnd::TrailingEof,
            max_carve_size: 256 * 1024 * 1024,
        },
        FileSignature {
            id: "mp4",
            extension: "mp4",
            mime: "video/mp4",
            magic: b"ftyp",
            magic_offset: 4,
            end: SignatureEnd::Mp4Moov,
            max_carve_size: 512 * 1024 * 1024,
        },
        FileSignature {
            id: "docx",
            extension: "docx",
            mime: "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            magic: b"PK\x03\x04",
            magic_offset: 0,
            end: SignatureEnd::ZipEocd,
            max_carve_size: 512 * 1024 * 1024,
        },
        FileSignature {
            id: "zip",
            extension: "zip",
            mime: "application/zip",
            magic: b"PK\x03\x04",
            magic_offset: 0,
            end: SignatureEnd::ZipEocd,
            max_carve_size: 512 * 1024 * 1024,
        },
    ]
}
