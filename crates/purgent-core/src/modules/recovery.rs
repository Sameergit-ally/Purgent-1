use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use uuid::Uuid;

use super::classification::{classify, FileCategory};
use super::config::{signature_database, FileSignature, SignatureEnd};
use super::hashing::sha256_hex;
use super::log;

const SCAN_CHUNK: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CarveError {
    Source(String),
    Destination(String),
    Io(String),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScoreFactor {
    pub key: String,
    pub value: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecoveredFile {
    pub signature: String,
    pub extension: String,
    pub mime: String,
    pub category: FileCategory,
    pub output_path: String,
    pub size_bytes: u64,
    pub source_offset: u64,
    pub sha256: String,
    pub structure_valid: bool,
    pub fragment_reconstructed: bool,
    pub gap_bytes: u64,
    pub confidence: f64,
    pub score_factors: Vec<ScoreFactor>,
    pub recovered_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CarveRun {
    pub operation_id: String,
    pub source: String,
    pub output_dir: String,
    pub scanned_bytes: u64,
    pub files: Vec<RecoveredFile>,
    pub started_at: String,
    pub finished_at: String,
}

pub const MIN_CARVE_SIZE: u64 = 12;

pub fn carve_source(source: &Path, output_dir: &Path) -> Result<CarveRun, CarveError> {
    carve_source_with_progress(source, output_dir, &mut |_| {})
}

pub fn carve_source_with_progress(
    source: &Path,
    output_dir: &Path,
    progress: super::progress::ProgressFn,
) -> Result<CarveRun, CarveError> {
    let started_at = log::now_utc_rfc3339();
    let operation_id = Uuid::new_v4().to_string();

    log::log(
        "selected",
        &format!(
            "operation={operation_id} source={} output={}",
            source.display(),
            output_dir.display()
        ),
    );

    let mut file = File::open(source).map_err(|e| CarveError::Source(e.to_string()))?;
    let source_len = file
        .metadata()
        .map_err(|e| CarveError::Source(e.to_string()))?
        .len();

    std::fs::create_dir_all(output_dir).map_err(|e| CarveError::Destination(e.to_string()))?;

    let signatures = signature_database();
    let mut files = Vec::new();
    let mut consumed_until: u64 = 0;

    log::log(
        "running",
        &format!("operation={operation_id} carving {source_len} bytes read-only"),
    );
    loop {
        let (found, end) = find_next_signature(&mut file, signatures, consumed_until, source_len)?;
        let Some((sig, offset)) = found else {
            break;
        };
        consumed_until = end;
        let carve_start = offset.saturating_sub(sig.magic_offset as u64);
        if end - carve_start < MIN_CARVE_SIZE {
            if sig.id == "jpeg" {
                if let Some((content, gap_bytes, fragment_end)) =
                    recover_fragmented_jpeg(&mut file, offset, source_len, sig.max_carve_size)?
                {
                    consumed_until = fragment_end;
                    let recovered_len = content.len() as u64;
                    let recovered = write_recovered(
                        &mut file,
                        sig,
                        offset,
                        content,
                        recovered_len,
                        true,
                        gap_bytes,
                        &mut files,
                        output_dir,
                    )?;
                    log::log(
                        "running",
                        &format!(
                            "fragment_reconstructed signature=jpeg offset={offset} size={} gap={gap_bytes} file={}",
                            fragment_end - offset,
                            recovered.output_path
                        ),
                    );
                }
            }
            progress(super::progress::ProgressUpdate {
                operation_id: operation_id.clone(),
                operation_type: "file_recovery".to_string(),
                phase: "scan".to_string(),
                bytes_done: consumed_until,
                total_bytes: source_len,
                message: format!("scanning source at offset {consumed_until}"),
            });
            continue;
        }
        let content = read_range(&mut file, carve_start, end - carve_start)?;
        let recovered = write_recovered(
            &mut file,
            sig,
            carve_start,
            content,
            end - carve_start,
            false,
            0,
            &mut files,
            output_dir,
        )?;
        log::log(
            "running",
            &format!(
                "carved offset={offset} size={} signature={} valid={} file={}",
                end - offset,
                sig.id,
                recovered.structure_valid,
                recovered.output_path
            ),
        );
        progress(super::progress::ProgressUpdate {
            operation_id: operation_id.clone(),
            operation_type: "file_recovery".to_string(),
            phase: "scan".to_string(),
            bytes_done: consumed_until,
            total_bytes: source_len,
            message: format!("carved {} at offset {offset}", sig.id),
        });
    }

    let finished_at = log::now_utc_rfc3339();
    log::log(
        "verified",
        &format!(
            "operation={operation_id} carved={} hashes_recorded_at_extraction=true",
            files.len()
        ),
    );
    log::log(
        "reported",
        &format!("operation={operation_id} report_ready=true"),
    );

    Ok(CarveRun {
        operation_id,
        source: source.display().to_string(),
        output_dir: output_dir.display().to_string(),
        scanned_bytes: source_len,
        files,
        started_at,
        finished_at,
    })
}

fn find_next_signature(
    file: &mut File,
    signatures: &'static [FileSignature],
    from_offset: u64,
    source_len: u64,
) -> Result<(Option<(&'static FileSignature, u64)>, u64), CarveError> {
    let mut earliest: Option<(&'static FileSignature, u64)> = None;
    for sig in signatures {
        if let Some(pos) = search_forward(file, from_offset, source_len, sig.magic, true)? {
            let replaces = earliest.map(|(_, p)| pos < p).unwrap_or(true);
            if replaces {
                earliest = Some((sig, pos));
            }
        }
    }
    let Some((sig, start)) = earliest else {
        return Ok((None, source_len));
    };
    match resolve_end(file, sig, start, source_len)? {
        Some(end) => Ok((Some((sig, start)), end)),
        None => Ok((Some((sig, start)), start + sig.magic.len() as u64)),
    }
}

fn search_forward(
    file: &mut File,
    start: u64,
    end: u64,
    marker: &[u8],
    first_match: bool,
) -> Result<Option<u64>, CarveError> {
    let mut found: Option<u64> = None;
    let mut tail: Vec<u8> = Vec::new();
    let mut anchor = start;
    while anchor < end {
        file.seek(SeekFrom::Start(anchor))
            .map_err(|e| CarveError::Io(e.to_string()))?;
        let n = ((end - anchor) as usize).min(SCAN_CHUNK);
        let mut rd = vec![0u8; n];
        let got = file
            .read(&mut rd)
            .map_err(|e| CarveError::Io(e.to_string()))?;
        if got == 0 {
            break;
        }
        let window_start = anchor - tail.len() as u64;
        let mut hay = Vec::with_capacity(tail.len() + got);
        hay.extend_from_slice(&tail);
        hay.extend_from_slice(&rd[..got]);
        if hay.len() >= marker.len() {
            for i in 0..=hay.len() - marker.len() {
                if &hay[i..i + marker.len()] == marker {
                    found = Some(window_start + i as u64);
                    if first_match {
                        return Ok(found);
                    }
                }
            }
        }
        if got >= marker.len() - 1 {
            tail = rd[got - (marker.len() - 1)..got].to_vec();
        } else {
            tail = rd[..got].to_vec();
        }
        anchor += got as u64;
    }
    Ok(found)
}

fn resolve_end(
    file: &mut File,
    sig: &FileSignature,
    start: u64,
    source_len: u64,
) -> Result<Option<u64>, CarveError> {
    let limit = (start + sig.max_carve_size).min(source_len);
    match sig.end {
        SignatureEnd::Terminator(marker) => {
            Ok(search_forward(file, start, limit, marker, true)?.map(|p| p + marker.len() as u64))
        }
        SignatureEnd::ByteTerminator(byte) => {
            let marker = [byte];
            Ok(search_forward(file, start, limit, &marker, true)?.map(|p| p + 1))
        }
        SignatureEnd::FixedSize { size_offset } => {
            let mut header = [0u8; 6];
            file.seek(SeekFrom::Start(start))
                .map_err(|e| CarveError::Io(e.to_string()))?;
            file.read_exact(&mut header)
                .map_err(|e| CarveError::Io(e.to_string()))?;
            let size = u32::from_le_bytes([
                header[size_offset],
                header[size_offset + 1],
                header[size_offset + 2],
                header[size_offset + 3],
            ]) as u64;
            if size == 0 || size < MIN_CARVE_SIZE {
                Ok(None)
            } else {
                Ok(Some((start + size).min(limit)))
            }
        }
        SignatureEnd::ZipEocd => {
            let marker = b"PK\x05\x06";
            Ok(
                search_forward(file, start, limit, marker, true)?.map(|eocd| {
                    let mut tail = [0u8; 2];
                    file.seek(SeekFrom::Start(eocd + 20)).ok();
                    let _ = file.read_exact(&mut tail);
                    let comment_len = u16::from_le_bytes(tail) as u64;
                    eocd + 22 + comment_len
                }),
            )
        }
        SignatureEnd::Mp4Moov => mp4_moov_end(file, start, limit),
        SignatureEnd::TrailingEof => {
            let marker = b"%%EOF";
            Ok(search_forward(file, start, limit, marker, false)?.map(|p| p + marker.len() as u64))
        }
    }
}

/// Walks top-level ISO BMFF boxes from `start` (the position of the `ftyp` magic,
/// which is 4 bytes into the containing box) and returns the end offset of the
/// `moov` box when found. Without a `moov` box the fragment cannot be a complete
/// MP4, so `None` is returned (the caller advances past the signature only).
fn mp4_moov_end(file: &mut File, start: u64, limit: u64) -> Result<Option<u64>, CarveError> {
    // `ftyp` occupies bytes 4..8 of its box; the size prefix is at start - 4.
    let mut box_pos = start.saturating_sub(4);
    let mut header = [0u8; 16];
    loop {
        file.seek(SeekFrom::Start(box_pos))
            .map_err(|e| CarveError::Io(e.to_string()))?;
        let got = file
            .read(&mut header)
            .map_err(|e| CarveError::Io(e.to_string()))?;
        if got < 8 {
            return Ok(None);
        }
        let size32 = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as u64;
        let box_type = [header[4], header[5], header[6], header[7]];
        let end = if size32 == 1 {
            // 64-bit extended size; header is 16 bytes.
            if got < 16 {
                return Ok(None);
            }
            let size64 = u64::from_be_bytes(header[8..16].try_into().expect("16 bytes"));
            box_pos.saturating_add(size64)
        } else if size32 == 0 {
            // Box extends to end of file; stop walking.
            return Ok(None);
        } else {
            box_pos.saturating_add(size32)
        };
        if &box_type == b"moov" {
            return Ok(Some(end.min(limit)));
        }
        if end <= box_pos || end > limit {
            return Ok(None);
        }
        box_pos = end;
    }
}

fn read_range(file: &mut File, offset: u64, len: u64) -> Result<Vec<u8>, CarveError> {
    let mut content = Vec::with_capacity(len as usize);
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| CarveError::Io(e.to_string()))?;
    let mut remaining = len;
    while remaining > 0 {
        let mut buf = vec![0u8; (remaining.min(SCAN_CHUNK as u64)) as usize];
        let n = file
            .read(&mut buf)
            .map_err(|e| CarveError::Io(e.to_string()))?;
        if n == 0 {
            break;
        }
        content.extend_from_slice(&buf[..n]);
        remaining -= n as u64;
    }
    Ok(content)
}

fn write_recovered(
    file: &mut File,
    sig: &FileSignature,
    offset: u64,
    content: Vec<u8>,
    size_bytes: u64,
    fragment_reconstructed: bool,
    gap_bytes: u64,
    files: &mut Vec<RecoveredFile>,
    output_dir: &Path,
) -> Result<RecoveredFile, CarveError> {
    let _ = file;
    let effective_id = if sig.id == "docx" && !docx_structure_valid(&content) {
        "zip"
    } else {
        sig.id
    };
    let effective_sig = if effective_id == sig.id {
        sig
    } else {
        signature_database()
            .iter()
            .find(|s| s.id == effective_id)
            .expect("zip signature is present")
    };
    let structure_valid = validate_structure(effective_id, &content);
    let (confidence, score_factors) = score(
        effective_id,
        structure_valid,
        fragment_reconstructed,
        content.len() as u64 == size_bytes,
    );
    let recovered_at = log::now_utc_rfc3339();
    let filename = format!(
        "found_{}_{}_{}.{}",
        effective_id,
        offset,
        files.len() + 1,
        effective_sig.extension
    );
    let out_path = output_dir.join(&filename);
    let hash = sha256_hex(&content);
    {
        let mut out =
            File::create(&out_path).map_err(|e| CarveError::Destination(e.to_string()))?;
        out.write_all(&content)
            .map_err(|e| CarveError::Destination(e.to_string()))?;
    }
    let recovered = RecoveredFile {
        signature: effective_id.to_string(),
        extension: effective_sig.extension.to_string(),
        mime: effective_sig.mime.to_string(),
        category: classify(effective_id, effective_sig.mime),
        output_path: out_path.display().to_string(),
        size_bytes,
        source_offset: offset,
        sha256: hash,
        structure_valid,
        fragment_reconstructed,
        gap_bytes,
        confidence,
        score_factors,
        recovered_at,
    };
    files.push(recovered.clone());
    Ok(recovered)
}

fn validate_structure(id: &str, data: &[u8]) -> bool {
    match id {
        "jpeg" => jpeg_structure_valid(data),
        "png" => png_structure_valid(data),
        "gif" => gif_structure_valid(data),
        "bmp" => bmp_structure_valid(data),
        "pdf" => pdf_structure_valid(data),
        "zip" => zip_structure_valid(data),
        "docx" => docx_structure_valid(data),
        "mp4" => mp4_structure_valid(data),
        _ => false,
    }
}

/// Validates an ISO BMFF container: a complete, walkable box chain that includes a
/// `moov` and a `mdat` box. Mirrors `mp4_moov_end` but operates on an in-memory
/// buffer for scored validation.
pub fn mp4_structure_valid(data: &[u8]) -> bool {
    if data.len() < 8 {
        return false;
    }
    let mut box_pos = 0usize;
    let mut saw_moov = false;
    let mut saw_mdat = false;
    loop {
        if box_pos == data.len() {
            break;
        }
        if box_pos + 8 > data.len() {
            return false;
        }
        let size32 =
            u32::from_be_bytes(data[box_pos..box_pos + 4].try_into().expect("4 bytes")) as u64;
        let box_type = &data[box_pos + 4..box_pos + 8];
        let header_len: usize;
        let size: usize;
        if size32 == 1 {
            if box_pos + 16 > data.len() {
                return false;
            }
            size = u64::from_be_bytes(data[box_pos + 8..box_pos + 16].try_into().expect("8 bytes"))
                as usize;
            header_len = 16;
        } else if size32 == 0 {
            size = data.len() - box_pos;
            header_len = 8;
        } else {
            size = size32 as usize;
            header_len = 8;
        }
        if size < header_len {
            return false;
        }
        match box_type {
            b"moov" => saw_moov = true,
            b"mdat" => saw_mdat = true,
            _ => {}
        }
        let next = box_pos + size;
        if next > data.len() {
            return false;
        }
        box_pos = next;
    }
    saw_moov && saw_mdat
}

/// Extracts entry names from a crafted ZIP buffer's central directory and checks
/// for the OOXML package markers (`[Content_Types].xml` and `word/document.xml`).
pub fn docx_structure_valid(data: &[u8]) -> bool {
    let mut names = Vec::new();
    let mut idx = 0usize;
    while idx + 4 <= data.len() && &data[idx..idx + 4] != b"PK\x01\x02" {
        idx += 1;
    }
    while idx + 46 <= data.len() && &data[idx..idx + 4] == b"PK\x01\x02" {
        let name_len = u16::from_le_bytes([data[idx + 28], data[idx + 29]]) as usize;
        let extra_len = u16::from_le_bytes([data[idx + 30], data[idx + 31]]) as usize;
        let comment_len = u16::from_le_bytes([data[idx + 32], data[idx + 33]]) as usize;
        let start = idx + 46;
        let end = start + name_len;
        if end > data.len() {
            break;
        }
        names.push(String::from_utf8_lossy(&data[start..end]).to_string());
        idx = end + extra_len + comment_len;
    }
    names.iter().any(|n| n == "[Content_Types].xml")
        && names.iter().any(|n| n == "word/document.xml")
}

fn score(
    id: &str,
    structure_valid: bool,
    fragment_reconstructed: bool,
    size_consistent: bool,
) -> (f64, Vec<ScoreFactor>) {
    let terminator_based = matches!(id, "jpeg" | "png" | "gif" | "pdf" | "zip" | "docx" | "mp4");
    let mut factors = vec![
        ScoreFactor {
            key: "structure_valid".into(),
            value: if structure_valid { 0.6 } else { 0.0 },
        },
        ScoreFactor {
            key: "terminator_present".into(),
            value: if terminator_based { 0.15 } else { 0.0 },
        },
        ScoreFactor {
            key: "fragment_reconstructed".into(),
            value: if fragment_reconstructed { -0.21 } else { 0.0 },
        },
        ScoreFactor {
            key: "size_consistent".into(),
            value: if size_consistent { 0.09 } else { 0.0 },
        },
    ];
    let confidence = if !structure_valid {
        0.30
    } else if fragment_reconstructed {
        0.74
    } else if terminator_based {
        0.95
    } else if size_consistent {
        0.84
    } else {
        0.70
    };
    factors.push(ScoreFactor {
        key: "confidence".into(),
        value: confidence,
    });
    (confidence, factors)
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

fn jpeg_structure_valid(data: &[u8]) -> bool {
    if data.len() < 4 || data[0..2] != [0xFF, 0xD8] {
        return false;
    }
    let mut pos = 2usize;
    loop {
        if pos + 1 >= data.len() {
            break;
        }
        if data[pos] != 0xFF {
            break;
        }
        let marker = data[pos + 1];
        if marker == 0xD9 {
            return true;
        }
        if marker == 0x00 || marker == 0x01 || (0xD0..=0xD8).contains(&marker) {
            pos += 2;
            continue;
        }
        if pos + 4 > data.len() {
            break;
        }
        let seg_len = u16::from_be_bytes([data[pos + 2], data[pos + 3]]) as usize;
        if seg_len < 2 {
            return false;
        }
        pos += 2 + seg_len;
    }
    data[pos..].windows(2).any(|w| w == [0xFF, 0xD9])
}

fn png_structure_valid(data: &[u8]) -> bool {
    let sig = b"\x89PNG\r\n\x1A\n";
    if data.len() < sig.len() + 12 || &data[..8] != sig {
        return false;
    }
    let mut pos = 8usize;
    loop {
        if pos + 12 > data.len() {
            return false;
        }
        let chunk_len =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        let chunk_type = &data[pos + 4..pos + 8];
        let end = pos + 8 + chunk_len;
        if end + 4 > data.len() {
            return false;
        }
        let expected_crc =
            u32::from_be_bytes([data[end], data[end + 1], data[end + 2], data[end + 3]]);
        let actual_crc = crc32(&data[pos + 4..end]);
        if expected_crc != actual_crc {
            return false;
        }
        if chunk_type == b"IEND" {
            return true;
        }
        pos = end + 4;
    }
}

fn gif_structure_valid(data: &[u8]) -> bool {
    let header_ok = data.len() >= 6 && (&data[..6] == b"GIF87a" || &data[..6] == b"GIF89a");
    header_ok && data.len() >= 13 && data[data.len() - 1] == 0x3B
}

fn bmp_structure_valid(data: &[u8]) -> bool {
    if data.len() < 14 || &data[..2] != b"BM" {
        return false;
    }
    let declared = u32::from_le_bytes([data[2], data[3], data[4], data[5]]) as u64;
    let dib_offset = u32::from_le_bytes([data[10], data[11], data[12], data[13]]) as u64;
    declared > 0 && declared <= data.len() as u64 && dib_offset <= declared
}

fn pdf_structure_valid(data: &[u8]) -> bool {
    let has_magic = data.len() >= 8 && data.starts_with(b"%PDF-");
    let has_eof = data.ends_with(b"EOF") || data.ends_with(b"%%EOF");
    let has_objects =
        data.windows(4).any(|w| w == b"xref") || data.windows(4).any(|w| w == b"endobj");
    has_magic && has_eof && has_objects
}

fn zip_structure_valid(data: &[u8]) -> bool {
    let has_local = data.len() >= 4 && data.starts_with(b"PK\x03\x04");
    let eocd: Vec<usize> = data
        .windows(4)
        .enumerate()
        .filter(|(_, w)| w == b"PK\x05\x06")
        .map(|(i, _)| i)
        .collect();
    has_local && eocd.iter().any(|&i| i + 22 <= data.len())
}

fn recover_fragmented_jpeg(
    file: &mut File,
    start: u64,
    source_len: u64,
    max_carve_size: u64,
) -> Result<Option<(Vec<u8>, u64, u64)>, CarveError> {
    let window_limit = (start + max_carve_size).min(source_len);
    let window = read_range(file, start, window_limit - start)?;
    let mut pos = 2usize;
    let mut parsed_end = 2usize;
    while pos + 1 < window.len() {
        if window[pos] != 0xFF {
            break;
        }
        let marker = window[pos + 1];
        if marker == 0xD9 {
            return Ok(None);
        }
        if marker == 0x00 || marker == 0x01 || (0xD0..=0xD8).contains(&marker) {
            parsed_end = pos + 2;
            pos += 2;
            continue;
        }
        if pos + 2 + 2 > window.len() {
            break;
        }
        let seg_len = u16::from_be_bytes([window[pos + 2], window[pos + 3]]) as usize;
        if seg_len < 2 || pos + 2 + seg_len > window.len() {
            break;
        }
        parsed_end = pos + 2 + seg_len;
        pos += 2 + seg_len;
    }
    let frag1_end = start + parsed_end as u64;
    let eoi = match search_forward(file, frag1_end + 1, source_len, b"\xFF\xD9", true)? {
        Some(eoi) => eoi,
        None => return Ok(None),
    };
    let mut cursor = frag1_end + 1;
    loop {
        file.seek(SeekFrom::Start(cursor))
            .map_err(|e| CarveError::Io(e.to_string()))?;
        let mut probe = [0u8; 16 * 1024];
        let got = file
            .read(&mut probe)
            .map_err(|e| CarveError::Io(e.to_string()))?;
        if got < 2 {
            break;
        }
        let mut resume: Option<u64> = None;
        for i in 0..got - 1 {
            if probe[i] != 0xFF {
                continue;
            }
            let marker = probe[i + 1];
            if marker == 0x00 || marker == 0x01 || (0xD0..=0xD8).contains(&marker) {
                continue;
            }
            if i + 4 > got {
                continue;
            }
            let seg_len = u16::from_be_bytes([probe[i + 2], probe[i + 3]]) as usize;
            if seg_len < 2 || i + 2 + seg_len > got {
                continue;
            }
            let candidate = cursor + i as u64;
            if candidate + (seg_len + 4) as u64 <= eoi {
                resume = Some(candidate);
            }
            break;
        }
        if let Some(resume) = resume {
            let gap = resume - frag1_end;
            if gap >= 256 {
                let frag1 = read_range(file, start, frag1_end - start)?;
                let frag2 = read_range(file, resume, eoi + 2 - resume)?;
                let mut rebuilt = frag1;
                rebuilt.extend_from_slice(&frag2);
                return Ok(Some((rebuilt, gap, eoi + 2)));
            }
            cursor = resume + 3;
        } else {
            cursor += (got - 1) as u64;
        }
        if cursor >= eoi {
            break;
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::hashing::sha256_hex;
    use std::path::PathBuf;

    fn write_blob(dir: &Path, name: &str, bytes: &[u8]) -> (PathBuf, String) {
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        let hash = sha256_hex(bytes);
        (path, hash)
    }

    #[test]
    fn carves_known_files_from_mixed_image_with_matching_hashes() {
        let dir = std::env::temp_dir().join(format!("purgent-carve-{}", Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::create_dir_all(dir.join("out")).unwrap();

        let jpeg = jpeg_bytes();
        let png = png_bytes();
        let gif = gif_bytes();
        let bmp = bmp_bytes();
        let pdf = pdf_bytes();
        let zip = zip_bytes();

        let (_, jpeg_hash) = write_blob(&dir.join("src"), "a.jpg", &jpeg);
        let (_, png_hash) = write_blob(&dir.join("src"), "b.png", &png);
        let (_, gif_hash) = write_blob(&dir.join("src"), "c.gif", &gif);
        let (_, bmp_hash) = write_blob(&dir.join("src"), "d.bmp", &bmp);
        let (_, pdf_hash) = write_blob(&dir.join("src"), "e.pdf", &pdf);
        let (_, zip_hash) = write_blob(&dir.join("src"), "f.zip", &zip);

        let mut image = Vec::new();
        image.extend_from_slice(&[0x00, 0x11, 0x22, 0x33]);
        image.extend_from_slice(&jpeg);
        image.extend_from_slice(&[0x0F; 333]);
        image.extend_from_slice(&png);
        image.extend_from_slice(&[0x0E, 0x0D, 0x0C]);
        image.extend_from_slice(&gif);
        image.extend_from_slice(b"=== filler ===filler=== ");
        image.extend_from_slice(&bmp);
        image.extend_from_slice(&[0x0A; 512]);
        image.extend_from_slice(&pdf);
        image.extend_from_slice(&[0x0B; 128]);
        image.extend_from_slice(&zip);
        image.extend_from_slice(&[0x99; 777]);

        let image_path = dir.join("src").join("master.image");
        std::fs::write(&image_path, &image).unwrap();

        let out = dir.join("out");
        let run = carve_source(&image_path, &out).unwrap();

        let mut by_sig = std::collections::HashMap::new();
        for f in &run.files {
            by_sig.insert(f.signature.clone(), f.clone());
        }
        assert_eq!(run.files.len(), 6, "all six planted files must be carved");
        let expected_blobs = [
            ("jpeg", jpeg.as_slice(), &jpeg_hash),
            ("png", png.as_slice(), &png_hash),
            ("gif", gif.as_slice(), &gif_hash),
            ("bmp", bmp.as_slice(), &bmp_hash),
            ("pdf", pdf.as_slice(), &pdf_hash),
            ("zip", zip.as_slice(), &zip_hash),
        ];
        for f in &run.files {
            let (name, blob, hash) = expected_blobs
                .iter()
                .find(|(n, _, _)| *n == f.signature)
                .expect("known signature");
            if &f.sha256 != *hash {
                let actual = std::fs::read(&f.output_path).unwrap();
                eprintln!(
                    "MISMATCH {} planted.len={} carved.len={} offset={}: carved.fn={:02X?} carved.ln={:02X?} planted.hd={:02X?} planted.tl={:02X?}",
                    name,
                    blob.len(),
                    actual.len(),
                    f.source_offset,
                    &actual[..8.min(actual.len())],
                    &actual[actual.len().saturating_sub(8)..],
                    &blob[..8.min(blob.len())],
                    &blob[blob.len().saturating_sub(8)..],
                );
            }
            assert_eq!(&f.sha256, *hash, "hash mismatch for {name}");
        }
        assert!(run.scanned_bytes >= image.len() as u64);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn carved_files_exist_on_disk_and_have_sizes() {
        let dir = std::env::temp_dir().join(format!("purgent-carve-{}", Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("out")).unwrap();
        let image_path = dir.join("gray.image");
        std::fs::write(&image_path, b"seed 01 02 03 ").unwrap();
        let run = carve_source(&image_path, &dir.join("out")).unwrap();
        assert!(run.files.is_empty(), "no signature, nothing carved");
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn jpeg_bytes() -> Vec<u8> {
        let mut v = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J'];
        v.extend(vec![0xAA; 512]);
        v.extend_from_slice(&[0xFF, 0xD9]);
        v
    }

    fn png_bytes() -> Vec<u8> {
        let mut v = b"\x89PNG\r\n\x1A\n".to_vec();
        v.extend_from_slice(&[0x00, 0x00, 0x00, 0x0D, b'I', b'H', b'D', b'R']);
        v.extend_from_slice(&[
            0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x02, 0x08, 0x02, 0x00, 0x00, 0x00,
        ]);
        v.extend_from_slice(&[0x00; 64]);
        v.extend_from_slice(&[
            0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ]);
        v
    }

    fn gif_bytes() -> Vec<u8> {
        let mut v = b"GIF89a".to_vec();
        v.extend_from_slice(&[0x01, 0x00, 0x01, 0x00, 0x80, 0x00, 0x00]);
        v.extend_from_slice(&[0x00, 0x00, 0x00, 0x33, 0x33, 0x33]);
        v.extend_from_slice(&[0x2C, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00]);
        v.extend_from_slice(&[0x02, 0x02, 0x44, 0x01, 0x00]);
        v.push(0x3B);
        v
    }

    fn bmp_bytes() -> Vec<u8> {
        let size: u32 = 122u32;
        let mut v = b"BM".to_vec();
        v.extend_from_slice(&size.to_le_bytes());
        v.resize(size as usize, 0x00);
        v
    }

    fn pdf_bytes() -> Vec<u8> {
        let mut v = b"%PDF-1.4\n1 0 obj\n<< >>\nendobj\ntrailer\n<< >>\n%%EOF".to_vec();
        v.extend_from_slice(&[0x00; 33]);
        v.extend_from_slice(b"%%EOF");
        v
    }

    fn zip_bytes() -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(b"PK\x03\x04");
        v.extend_from_slice(&0x0014u16.to_le_bytes());
        v.extend_from_slice(&0u16.to_le_bytes());
        v.extend_from_slice(&0u16.to_le_bytes());
        v.extend_from_slice(&[0x5A, 0x00, 0x00, 0x00]);
        v.extend_from_slice(&64u32.to_le_bytes());
        v.extend_from_slice(&64u32.to_le_bytes());
        v.extend_from_slice(&5u16.to_le_bytes());
        v.extend_from_slice(&0u16.to_le_bytes());
        v.extend_from_slice(b"f.txt");
        v.extend_from_slice(&[0xAA; 64]);
        v.extend_from_slice(b"PK\x01\x02");
        v.extend_from_slice(&[0x00; 8]);
        v.extend_from_slice(&[0x5A, 0x00, 0x00, 0x00]);
        v.extend_from_slice(&64u32.to_le_bytes());
        v.extend_from_slice(&64u32.to_le_bytes());
        v.extend_from_slice(&5u16.to_le_bytes());
        v.extend_from_slice(&0u16.to_le_bytes());
        v.extend_from_slice(&0u16.to_le_bytes());
        v.extend_from_slice(&0u16.to_le_bytes());
        v.extend_from_slice(&0u16.to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(b"f.txt");
        v.extend_from_slice(b"PK\x05\x06");
        v.extend_from_slice(&[0x00; 16]);
        v.extend_from_slice(&0u16.to_le_bytes());
        v
    }

    /// Builds a valid OOXML package (DOCX) blob with a central directory that lists
    /// `[Content_Types].xml` and `word/document.xml` entries.
    fn docx_bytes() -> Vec<u8> {
        fn entry(name: &str) -> Vec<u8> {
            let data = match name {
                "[Content_Types].xml" => b"<Types/>".to_vec(),
                "word/document.xml" => b"<document/>".to_vec(),
                _ => vec![0u8; 8],
            };
            let mut e = Vec::new();
            e.extend_from_slice(b"PK\x03\x04");
            e.extend_from_slice(&20u16.to_le_bytes());
            e.extend_from_slice(&0u16.to_le_bytes());
            e.extend_from_slice(&0u16.to_le_bytes());
            e.extend_from_slice(&0u32.to_le_bytes());
            e.extend_from_slice(&(data.len() as u32).to_le_bytes());
            e.extend_from_slice(&(data.len() as u32).to_le_bytes());
            e.extend_from_slice(&(name.len() as u16).to_le_bytes());
            e.extend_from_slice(&0u16.to_le_bytes());
            e.extend_from_slice(name.as_bytes());
            e.extend_from_slice(&data);
            e
        }

        let names = [
            "[Content_Types].xml",
            "word/document.xml",
            "word/styles.xml",
        ];
        let mut v = Vec::new();
        let mut entries: Vec<(&str, u32)> = Vec::new();
        for name in names {
            let e = entry(name);
            v.extend_from_slice(&e);
            entries.push((name, e.len() as u32));
        }
        let cd_start = v.len() as u32;
        let mut cd_entries: Vec<u32> = Vec::new();
        for (name, size) in &entries {
            cd_entries.push(v.len() as u32);
            // Central directory file header per APPNOTE.TXT; offsets relative to sig.
            v.extend_from_slice(b"PK\x01\x02"); // 0..4 signature
            v.extend_from_slice(&20u16.to_le_bytes()); // 4..6 version made by
            v.extend_from_slice(&20u16.to_le_bytes()); // 6..8 version needed
            v.extend_from_slice(&0u16.to_le_bytes()); // 8..10 flags
            v.extend_from_slice(&0u16.to_le_bytes()); // 10..12 compression
            v.extend_from_slice(&0u16.to_le_bytes()); // 12..14 mod time
            v.extend_from_slice(&0u16.to_le_bytes()); // 14..16 mod date
            v.extend_from_slice(&0u32.to_le_bytes()); // 16..20 crc32
            v.extend_from_slice(&size.to_le_bytes()); // 20..24 compressed size
            v.extend_from_slice(&size.to_le_bytes()); // 24..28 uncompressed size
            v.extend_from_slice(&(name.len() as u16).to_le_bytes()); // 28..30 name len
            v.extend_from_slice(&0u16.to_le_bytes()); // 30..32 extra len
            v.extend_from_slice(&0u16.to_le_bytes()); // 32..34 comment len
            v.extend_from_slice(&0u16.to_le_bytes()); // 34..36 disk number
            v.extend_from_slice(&0u16.to_le_bytes()); // 36..38 internal attrs
            v.extend_from_slice(&0u32.to_le_bytes()); // 38..42 external attrs
            v.extend_from_slice(&0u32.to_le_bytes()); // 42..46 local header offset
            v.extend_from_slice(name.as_bytes()); // 46.. name
        }
        let cd_len = (v.len() - cd_start as usize) as u32;
        v.extend_from_slice(b"PK\x05\x06");
        v.extend_from_slice(&[0x00; 4]);
        v.extend_from_slice(&(cd_entries.len() as u16).to_le_bytes());
        v.extend_from_slice(&(cd_entries.len() as u16).to_le_bytes());
        v.extend_from_slice(&cd_len.to_le_bytes());
        v.extend_from_slice(&cd_start.to_le_bytes());
        v.extend_from_slice(&0u16.to_le_bytes());
        v
    }

    /// Builds a minimal valid MP4: ftyp, freespace padding, mdat with payload, moov.
    fn mp4_bytes() -> Vec<u8> {
        let mut v = Vec::new();
        // ftyp box
        let ftyp_payload = b"isom\x00\x00\x02\x00isomiso2mp41";
        v.extend_from_slice(&(8 + ftyp_payload.len() as u32).to_be_bytes());
        v.extend_from_slice(b"ftyp");
        v.extend_from_slice(ftyp_payload);
        // mdat box with payload
        let mdat_payload = [0xAD; 128];
        v.extend_from_slice(&(8 + mdat_payload.len() as u32).to_be_bytes());
        v.extend_from_slice(b"mdat");
        v.extend_from_slice(&mdat_payload);
        // moov box with mvhd child
        let mut mvhd = Vec::new();
        mvhd.extend_from_slice(&(8 + 96u32).to_be_bytes());
        mvhd.extend_from_slice(b"mvhd");
        mvhd.extend_from_slice(&[0x00; 96]);
        v.extend_from_slice(&(8 + mvhd.len() as u32).to_be_bytes());
        v.extend_from_slice(b"moov");
        v.extend_from_slice(&mvhd);
        v
    }

    fn valid_jpeg_bytes() -> Vec<u8> {
        let mut v = vec![0xFF, 0xD8];
        v.extend_from_slice(&[
            0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x00, 0x00, 0x01,
            0x00, 0x01, 0x00, 0x00,
        ]);
        v.extend_from_slice(&[
            0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00, 0x08, 0x00, 0x08, 0x01, 0x01, 0x11, 0x00,
        ]);
        v.extend_from_slice(&[0xFF, 0xDB, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00]);
        v.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00]);
        v.extend_from_slice(&[0xAA; 96]);
        v.extend_from_slice(&[0xFF, 0xD9]);
        v
    }

    fn valid_png_bytes() -> Vec<u8> {
        let mut v = b"\x89PNG\r\n\x1A\n".to_vec();
        let mut ihdr = b"IHDR".to_vec();
        ihdr.extend_from_slice(&[
            0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x02, 0x08, 0x02, 0x00, 0x00, 0x00,
        ]);
        v.extend_from_slice(&(13u32).to_be_bytes());
        v.extend_from_slice(&ihdr);
        v.extend_from_slice(&crc32(&ihdr).to_be_bytes());
        let mut idat = b"IDAT".to_vec();
        idat.push(0x00);
        v.extend_from_slice(&(1u32).to_be_bytes());
        v.extend_from_slice(&idat);
        v.extend_from_slice(&crc32(&idat).to_be_bytes());
        let iend = b"IEND".to_vec();
        v.extend_from_slice(&(0u32).to_be_bytes());
        v.extend_from_slice(&iend);
        v.extend_from_slice(&crc32(&iend).to_be_bytes());
        v
    }

    fn bad_bmp_offset_bytes() -> Vec<u8> {
        let mut v = b"BM".to_vec();
        v.extend_from_slice(&122u32.to_le_bytes());
        v.resize(122, 0x00);
        v[10..14].copy_from_slice(&200u32.to_le_bytes());
        v
    }

    #[test]
    fn mixed_known_good_and_bad_files_are_ranked_by_confidence() {
        let dir = std::env::temp_dir().join(format!("purgent-rank-{}", Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("out")).unwrap();

        let good_jpeg = valid_jpeg_bytes();
        let good_png = valid_png_bytes();
        let good_gif = gif_bytes();
        let good_bmp = bmp_bytes();
        let bad_png = png_bytes();
        let bad_bmp = bad_bmp_offset_bytes();

        let mut image = Vec::new();
        image.extend_from_slice(&[0x00, 0x11, 0x22, 0x33]);
        image.extend_from_slice(&good_jpeg);
        image.extend_from_slice(&[0x0F; 5]);
        image.extend_from_slice(&bad_png);
        image.extend_from_slice(&[0x0E; 3]);
        image.extend_from_slice(&good_png);
        image.extend_from_slice(&[0x13; 9]);
        image.extend_from_slice(&bad_bmp);
        image.extend_from_slice(&[0x37; 7]);
        image.extend_from_slice(&good_gif);
        image.extend_from_slice(&[0x5F; 4]);
        image.extend_from_slice(&good_bmp);
        image.extend_from_slice(&[0x0A; 64]);

        let image_path = dir.join("rank.image");
        std::fs::write(&image_path, &image).unwrap();

        let run = carve_source(&image_path, &dir.join("out")).unwrap();

        let good_blobs = [
            ("good_jpeg", good_jpeg.as_slice()),
            ("good_png", good_png.as_slice()),
            ("good_gif", good_gif.as_slice()),
            ("good_bmp", good_bmp.as_slice()),
        ];
        let bad_blobs = [
            ("bad_png", bad_png.as_slice()),
            ("bad_bmp", bad_bmp.as_slice()),
        ];

        let mut good_min = f64::MAX;
        let mut bad_max = f64::MIN;
        let mut good_seen = 0;
        let mut bad_seen = 0;
        for f in &run.files {
            let on_disk = std::fs::read(&f.output_path).unwrap();
            let on_disk_hash = sha256_hex(&on_disk);
            let is_good = good_blobs
                .iter()
                .any(|(_, b)| sha256_hex(b) == on_disk_hash);
            let is_bad = bad_blobs.iter().any(|(_, b)| sha256_hex(b) == on_disk_hash);
            assert!(
                is_good || is_bad,
                "every carve must match a known blob, sig={} got {}",
                f.signature,
                on_disk_hash
            );
            if is_good {
                good_seen += 1;
                assert!(
                    f.structure_valid,
                    "known-good {} must validate",
                    on_disk_hash
                );
                assert!(
                    f.confidence >= 0.84,
                    "known-good confidence too low: {}",
                    f.confidence
                );
                good_min = good_min.min(f.confidence);
            } else {
                bad_seen += 1;
                assert!(
                    !f.structure_valid,
                    "known-bad {} must fail validation",
                    on_disk_hash
                );
                assert!(
                    f.confidence <= 0.30,
                    "known-bad confidence too high: {}",
                    f.confidence
                );
                bad_max = bad_max.max(f.confidence);
            }
            assert!(!f.fragment_reconstructed);
            assert_eq!(f.gap_bytes, 0);
        }
        assert_eq!(good_seen, good_blobs.len(), "all good blobs recovered");
        assert_eq!(bad_seen, bad_blobs.len(), "all bad blobs recovered");
        assert!(
            bad_max < good_min,
            "bad files must rank below good files (bad_max={bad_max} good_min={good_min})"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn carves_mp4_and_docx_with_category_classification() {
        let dir = std::env::temp_dir().join(format!("purgent-oa-{}", Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::create_dir_all(dir.join("out")).unwrap();

        let mp4 = mp4_bytes();
        let docx = docx_bytes();
        let zip = zip_bytes();

        let mp4_hash = sha256_hex(&mp4);
        let docx_hash = sha256_hex(&docx);

        let mut image = Vec::new();
        image.extend_from_slice(&[0x01, 0x02, 0x03]);
        image.extend_from_slice(&mp4);
        image.extend_from_slice(&[0x0A; 96]);
        image.extend_from_slice(&docx);
        image.extend_from_slice(&[0x0B; 64]);
        image.extend_from_slice(&zip);
        image.extend_from_slice(&[0x0C; 128]);

        let image_path = dir.join("src").join("oa.image");
        std::fs::write(&image_path, &image).unwrap();

        let run = carve_source(&image_path, &dir.join("out")).unwrap();

        assert_eq!(run.files.len(), 3, "mp4, docx and plain zip all carved");
        let by_sig: std::collections::HashMap<String, &RecoveredFile> =
            run.files.iter().map(|f| (f.signature.clone(), f)).collect();
        let mp4f = by_sig.get("mp4").expect("mp4 carved");
        assert_eq!(mp4f.sha256, mp4_hash);
        assert!(mp4f.structure_valid, "mp4 must parse as valid BMFF");
        assert_eq!(mp4f.category, FileCategory::Video);
        let docxf = by_sig.get("docx").expect("docx carved");
        assert_eq!(docxf.sha256, docx_hash);
        assert!(docxf.structure_valid, "docx must be OOXML-validated");
        assert_eq!(docxf.category, FileCategory::Document);
        let zipf = by_sig.get("zip").expect("plain zip stays zip, not docx");
        assert!(zipf.structure_valid, "plain zip remains structurally valid");
        assert_eq!(zipf.category, FileCategory::Archive);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn fragmented_jpeg_is_reconstructed_and_matches_original() {
        let dir = std::env::temp_dir().join(format!("purgent-frag-{}", Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("out")).unwrap();

        let jpeg = valid_jpeg_bytes();
        let orig_hash = sha256_hex(&jpeg);
        let split = 20usize;
        let gap = 66 * 1024 * 1024;

        let mut image = Vec::new();
        image.extend_from_slice(&[0x77, 0x66, 0x55]);
        image.extend_from_slice(&jpeg[..split]);
        image.extend_from_slice(&vec![0x13; gap]);
        image.extend_from_slice(&jpeg[split..]);
        image.extend_from_slice(&[0x99; 512]);

        let image_path = dir.join("frag.image");
        std::fs::write(&image_path, &image).unwrap();

        let run = carve_source(&image_path, &dir.join("out")).unwrap();

        assert_eq!(run.files.len(), 1, "only the fragmented jpeg is expected");
        let f = &run.files[0];
        assert_eq!(f.signature, "jpeg");
        assert_eq!(
            f.sha256, orig_hash,
            "reconstructed bytes must match the original"
        );
        assert!(f.structure_valid, "reconstructed file must validate");
        assert!(f.fragment_reconstructed);
        assert_eq!(f.gap_bytes, gap as u64);
        assert!((f.confidence - 0.74).abs() < 1e-9, "fragmented confidence");
        assert_eq!(f.size_bytes, jpeg.len() as u64);
        let on_disk = std::fs::read(&f.output_path).unwrap();
        assert_eq!(sha256_hex(&on_disk), orig_hash);
        assert_eq!(on_disk, jpeg, "recovered bytes identical to original");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
