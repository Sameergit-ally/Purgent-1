//! In-band hardware-backed erasure: ATA Secure Erase and NVMe Sanitize.
//!
//! Executed only through a `WipeTarget::Device`. Every failure path surfaces as a
//! distinct error that the caller must confirm explicitly before any overwrite
//! fallback is allowed (see `drive_eraser::WipeError::RequiresFallbackAck`) — there is
//! never a silent degradation from a hardware method to blind overwrite.

#[cfg(windows)]
use super::hpa_dco::ATA_FLAGS_DATA_OUT;
use super::hpa_dco::{query_hpa_dco, HpaDcoState};
use super::MediaType;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HardwareEraseError {
    Unsupported(String),
    Io(String),
}

impl std::fmt::Display for HardwareEraseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HardwareEraseError::Unsupported(msg) => write!(f, "{msg}"),
            HardwareEraseError::Io(msg) => write!(f, "{msg}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SecureEraseOutcome {
    pub method: &'static str,
    pub status: &'static str,
    pub detail: String,
    pub hpa_dco: HpaDcoState,
}

/// Requests an in-band hardware erase on `path` given the device's media type.
/// Returns `Err(HardwareEraseError)` for anything that cannot be satisfied, so the
/// caller can require an explicit operator acknowledgement before falling back.
pub fn hardware_erase(
    path: &str,
    media_type: MediaType,
) -> Result<SecureEraseOutcome, HardwareEraseError> {
    if matches!(
        media_type,
        MediaType::UsbFlash | MediaType::SdFlash | MediaType::CdRom
    ) {
        return Err(HardwareEraseError::Unsupported(format!(
            "{} media cannot receive an in-band ATA/NVMe erase command",
            media_type.label()
        )));
    }
    if media_type == MediaType::Nvme {
        #[cfg(target_os = "linux")]
        {
            nvme_sanitize_linux(path)
        }
        #[cfg(windows)]
        {
            nvme_sanitize_windows(path)
        }
        #[cfg(not(any(windows, target_os = "linux")))]
        {
            let _ = path;
            Err(HardwareEraseError::Unsupported(
                "NVMe Sanitize is not implemented on this platform build".into(),
            ))
        }
    } else {
        #[cfg(windows)]
        {
            ata_secure_erase_windows(path)
        }
        #[cfg(target_os = "linux")]
        {
            ata_secure_erase_linux(path)
        }
        #[cfg(not(any(windows, target_os = "linux")))]
        {
            let _ = path;
            Err(HardwareEraseError::Unsupported(
                "ATA Secure Erase is not implemented on this platform build".into(),
            ))
        }
    }
}

/// Best-effort HPA/DCO inspection for a device path.
pub fn hpa_dco_status(path: &str) -> Option<HpaDcoState> {
    query_hpa_dco(path)
}

// ---------------------------------------------------------------------------
// Windows
// ---------------------------------------------------------------------------

#[cfg(windows)]
fn nvme_sanitize_windows(path: &str) -> Result<SecureEraseOutcome, HardwareEraseError> {
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::Ioctl::IOCTL_STORAGE_PROTOCOL_COMMAND;
    use windows_sys::Win32::System::IO::DeviceIoControl;

    let wide: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            windows_sys::Win32::Foundation::GENERIC_READ
                | windows_sys::Win32::Foundation::GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(HardwareEraseError::Io(
            "cannot open NVMe controller (admin required)".into(),
        ));
    }

    // STORAGE_PROPERTY_QUERY + STORAGE_PROTOCOL_SPECIFIC_DATA_EXT, then the NVMe
    // DWORD fields. For NVMe Device Sanitize (opcode 0x80):
    //   - ProtocolDataRequestSubValue  = CDW0 (opcode)
    //   - ProtocolDataRequestSubValue2 = CDW10 (sanitize action = cryptographic erase)
    //   - ...3 = CDW11, 4 = CDW12, 5 = CDW13
    // No data buffer is transferred for the Sanitize command.
    const PROTOCOL_TYPE_NVME: u32 = 3;
    const DATA_TYPE_ADAPTER: u32 = 0;
    const OPCODE_SANITIZE: u32 = 0x80;
    const SANITIZE_ACTION_CRYPTO_ERASE: u32 = 0x2;

    let mut payload = [0u8; 256];
    payload[0..4].copy_from_slice(&PROTOCOL_TYPE_NVME.to_le_bytes()); // ProtocolType
    payload[4..8].copy_from_slice(&DATA_TYPE_ADAPTER.to_le_bytes()); // DataType
    payload[8..12].copy_from_slice(&0u32.to_le_bytes()); // ProtocolDataRequestValue (flags)
    payload[12..16].copy_from_slice(&OPCODE_SANITIZE.to_le_bytes()); // SubValue = CDW0
    payload[16..20].copy_from_slice(&0u32.to_le_bytes()); // ProtocolDataOffset
    payload[20..24].copy_from_slice(&0u32.to_le_bytes()); // ProtocolDataLength
    payload[24..28].copy_from_slice(&SANITIZE_ACTION_CRYPTO_ERASE.to_le_bytes()); // CDW10
    payload[28..32].copy_from_slice(&0u32.to_le_bytes()); // CDW11 (overwrite pattern, n/a)
    payload[32..36].copy_from_slice(&0u32.to_le_bytes()); // CDW12 (scope, n/a)
    payload[36..40].copy_from_slice(&0u32.to_le_bytes()); // CDW13

    let mut query = [0u8; 16];
    query[0..4].copy_from_slice(&50u32.to_le_bytes()); // StorageAdapterProtocolSpecificProperty
    query[4..8].copy_from_slice(&0u32.to_le_bytes()); // PropertyStandardQuery

    let mut combined = Vec::with_capacity(16 + 256);
    combined.extend_from_slice(&query);
    combined.extend_from_slice(&payload);

    let mut returned = 0u32;
    let ok = unsafe {
        DeviceIoControl(
            handle,
            IOCTL_STORAGE_PROTOCOL_COMMAND,
            combined.as_ptr() as *const _,
            (query.len() + 40) as u32,
            combined.as_mut_ptr() as *mut _,
            combined.len() as u32,
            &mut returned,
            std::ptr::null_mut(),
        )
    };
    unsafe { windows_sys::Win32::Foundation::CloseHandle(handle) };

    if ok == 0 {
        return Err(HardwareEraseError::Io(
            "NVMe sanitize command rejected by the controller".into(),
        ));
    }
    Ok(SecureEraseOutcome {
        method: "nvme_sanitize",
        status: "issued",
        detail: "NVMe Device Sanitize (cryptographic erase) accepted by controller".into(),
        hpa_dco: HpaDcoState::default(),
    })
}

#[cfg(windows)]
const ATA_SECURITY_ERASE_PASSWORD: &[u8; 32] = b"PURGENT-ATA-ERASE-2026-SANITIZE\x00";

#[cfg(windows)]
fn ata_secure_erase_windows(path: &str) -> Result<SecureEraseOutcome, HardwareEraseError> {
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };

    let wide: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            windows_sys::Win32::Foundation::GENERIC_READ
                | windows_sys::Win32::Foundation::GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(HardwareEraseError::Io(
            "cannot open ATA device (admin required)".into(),
        ));
    }

    let hpa = query_hpa_dco(path);

    // Security feature-set handshake: set password -> unlock -> erase-prepare -> erase.
    let cmds = [
        (0xF1u8, true),  // SECURITY SET PASSWORD
        (0xF2u8, true),  // SECURITY UNLOCK
        (0xF3u8, false), // SECURITY ERASE PREPARE
        (0xF4u8, true),  // SECURITY ERASE UNIT
    ];
    for (i, (cmd, with_data)) in cmds.iter().enumerate() {
        let timeout_ms: u32 = if *cmd == 0xF4 {
            3 * 60 * 60 * 1000
        } else {
            30_000
        };
        let mut task = [0u8; 8];
        task[7] = *cmd;
        let mut sector = [0u8; 512];
        if *with_data {
            sector[0] = 0x00; // user master password identifier
            sector[1..33].copy_from_slice(ATA_SECURITY_ERASE_PASSWORD);
        }
        let status = ata_task(
            handle,
            task,
            if *with_data { Some(&sector) } else { None },
            timeout_ms,
        )?;
        if status != 0 {
            let _ = unsafe { windows_sys::Win32::Foundation::CloseHandle(handle) };
            return Err(HardwareEraseError::Io(format!(
                "ATA command 0x{cmd:02X} failed with status 0x{status:02X} (step {})",
                i + 1
            )));
        }
    }
    unsafe { windows_sys::Win32::Foundation::CloseHandle(handle) };

    Ok(SecureEraseOutcome {
        method: "ata_secure_erase",
        status: "issued",
        detail: "ATA Secure Erase (security feature set) executed on device".into(),
        hpa_dco: hpa.unwrap_or_default(),
    })
}

#[cfg(windows)]
/// Maps whether a command carries a 512-byte sector out to the WDK data-direction
/// flags (ntddscsi.h). Data-out = 0x04; no data = 0x00. Data-in commands use
/// `ATA_FLAGS_DATA_IN` directly at the call site.
fn ata_flags(data_out: bool) -> u16 {
    if data_out {
        ATA_FLAGS_DATA_OUT
    } else {
        0
    }
}

#[cfg(windows)]
fn ata_task(
    handle: windows_sys::Win32::Foundation::HANDLE,
    task_file: [u8; 8],
    data_out: Option<&[u8; 512]>,
    timeout_ms: u32,
) -> Result<u8, HardwareEraseError> {
    use super::hpa_dco::{serialize_apt, AtaPassThroughEx, IOCTL_ATA_PASS_THROUGH};
    use windows_sys::Win32::System::IO::DeviceIoControl;

    let data_len = if data_out.is_some() { 512 } else { 0 };
    let header_size = size_of::<AtaPassThroughEx>();
    let mut buffer = vec![0u8; header_size + 512];
    let apt = AtaPassThroughEx {
        Length: header_size as u16,
        AtaFlags: ata_flags(data_out.is_some()),
        PathId: 0,
        TargetId: 0,
        Lun: 0,
        ReservedAsUchar: 0,
        DataTransferLength: data_len,
        TimeOutValue: timeout_ms / 1000,
        ReservedAsUlong: 0,
        DataBufferOffset: header_size as u32,
        PreviousTaskFile: [0; 8],
        CurrentTaskFile: task_file,
    };
    serialize_apt(&apt, &mut buffer[..header_size]);
    if let Some(data) = data_out {
        buffer[header_size..header_size + 512].copy_from_slice(data);
    }

    let mut returned = 0u32;
    let ok = unsafe {
        DeviceIoControl(
            handle,
            IOCTL_ATA_PASS_THROUGH,
            buffer.as_ptr() as *const _,
            header_size as u32,
            buffer.as_mut_ptr() as *mut _,
            buffer.len() as u32,
            &mut returned,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(HardwareEraseError::Io(
            "ATA pass-through ioctl failed".into(),
        ));
    }
    Ok(buffer[header_size - 2])
}

// ---------------------------------------------------------------------------
// Linux
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
const NVME_IOCTL_ADMIN_CMD_64: u64 = (3u64 << 30) | (b'N' as u64) << 8 | 0x41 | (72u64 << 16);
#[cfg(target_os = "linux")]
const NVME_IOCTL_ADMIN_CMD_32: u64 = (3u64 << 30) | (b'N' as u64) << 8 | 0x41 | (68u64 << 16);

#[cfg(target_os = "linux")]
fn nvme_sanitize_linux(path: &str) -> Result<SecureEraseOutcome, HardwareEraseError> {
    use super::secure_erase::secure_erase_linux::nvme_admin_cmd;
    use std::os::unix::fs::OpenOptionsExt;
    use std::os::unix::io::AsRawFd;

    let ioctl_code = if std::mem::size_of::<nvme_admin_cmd>() == 72 {
        NVME_IOCTL_ADMIN_CMD_64
    } else {
        NVME_IOCTL_ADMIN_CMD_32
    };

    // NVMe admin commands are issued on the controller (/dev/nvmeN), not the namespace.
    let controller = controller_path(path);
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(controller)
        .map_err(|e| {
            HardwareEraseError::Io(format!(
                "cannot open controller for {path} (root required): {e}"
            ))
        })?;

    let mut cmd = nvme_admin_cmd {
        cdw10: 0x2, // SANACT = cryptographic erase
        cdw11: 0,
        cdw12: 0,
        cdw13: 0,
        ..nvme_admin_cmd::new(0x80, 0xFFFFFFFF, 300_000)
    };
    let ret = unsafe { libc::ioctl(file.as_raw_fd(), ioctl_code as libc::c_ulong, &mut cmd) };
    if ret != 0 {
        return Err(HardwareEraseError::Io(format!(
            "NVMe sanitize ioctl failed (errno {})",
            std::io::Error::last_os_error()
        )));
    }
    Ok(SecureEraseOutcome {
        method: "nvme_sanitize",
        status: "issued",
        detail: "NVMe Device Sanitize (cryptographic erase) accepted by controller".into(),
        hpa_dco: super::hpa_dco::HpaDcoState::default(),
    })
}

#[cfg(target_os = "linux")]
fn controller_path(path: &str) -> String {
    // /dev/nvme0n1 -> /dev/nvme0
    let base = path.strip_prefix("/dev/nvme").and_then(|rest| {
        rest.chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse::<u32>()
            .ok()
    });
    match base {
        Some(n) => format!("/dev/nvme{n}"),
        None => path.to_string(),
    }
}

#[cfg(target_os = "linux")]
pub(crate) mod secure_erase_linux {
    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    pub struct nvme_admin_cmd {
        pub opcode: u8,
        pub flags: u8,
        pub rsvd1: u16,
        pub nsid: u32,
        pub cdw2: u32,
        pub cdw3: u32,
        pub metadata: u64,
        pub addr: u64,
        pub metadata_len: u32,
        pub data_len: u32,
        pub cdw10: u32,
        pub cdw11: u32,
        pub cdw12: u32,
        pub cdw13: u32,
        pub cdw14: u32,
        pub cdw15: u32,
        pub timeout_ms: u32,
        pub result: u32,
    }

    impl nvme_admin_cmd {
        pub fn new(opcode: u8, nsid: u32, timeout_ms: u32) -> Self {
            nvme_admin_cmd {
                opcode,
                flags: 0,
                rsvd1: 0,
                nsid,
                cdw2: 0,
                cdw3: 0,
                metadata: 0,
                addr: 0,
                metadata_len: 0,
                data_len: 0,
                cdw10: 0,
                cdw11: 0,
                cdw12: 0,
                cdw13: 0,
                cdw14: 0,
                cdw15: 0,
                timeout_ms,
                result: 0,
            }
        }
    }

    pub const NVME_IOCTL_ADMIN_CMD: u64 = (3u64 << 30)
        | (b'N' as u64) << 8
        | 0x41
        | (u64::try_from(std::mem::size_of::<nvme_admin_cmd>()).unwrap() << 16);
}

#[cfg(target_os = "linux")]
fn ata_secure_erase_linux(path: &str) -> Result<SecureEraseOutcome, HardwareEraseError> {
    use std::os::unix::fs::OpenOptionsExt;
    use std::os::unix::io::AsRawFd;

    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
        .map_err(|e| {
            HardwareEraseError::Io(format!(
                "cannot open {path} for ATA command (root required): {e}"
            ))
        })?;
    let hpa = query_hpa_dco(path);

    let password: Vec<u8> = "PURGENT-ATA-ERASE-2026-SANITIZE"
        .bytes()
        .chain([0u8; 32])
        .take(32)
        .collect();
    // hd_drive_cmd_hdr: command, sector_number, sector_count, feature (4 bytes) then data.
    let mut buf = vec![0u8; 4 + 512];
    // `libc` does not export HDIO_DRIVE_CMD; value from linux/`include/uapi/linux/hdreg.h`.
    const HDIO_DRIVE_CMD: libc::c_ulong = 0x031f;

    let step = |command: u8, feature: u8, data: Option<&[u8; 512]>| {
        buf.fill(0);
        buf[0] = command;
        buf[3] = feature;
        if let Some(d) = data {
            buf[4..4 + 512].copy_from_slice(d);
        }
        let ret =
            unsafe { libc::ioctl(file.as_raw_fd(), HDIO_DRIVE_CMD, buf.as_mut_ptr() as *mut _) };
        ret >= 0
    };

    let mut sector = [0u8; 512];
    sector[0] = 0x00;
    sector[1..33].copy_from_slice(&password);
    if !step(0xF1, 0x00, Some(&sector)) {
        return Err(HardwareEraseError::Io("ATA set-password failed".into()));
    }
    if !step(0xF2, 0x00, Some(&sector)) {
        return Err(HardwareEraseError::Io("ATA unlock failed".into()));
    }
    if !step(0xF3, 0x00, None) {
        return Err(HardwareEraseError::Io("ATA erase-prepare failed".into()));
    }
    if !step(0xF4, 0x00, Some(&sector)) {
        return Err(HardwareEraseError::Io("ATA erase-unit failed".into()));
    }

    Ok(SecureEraseOutcome {
        method: "ata_secure_erase",
        status: "issued",
        detail: "ATA Secure Erase (security feature set) executed on device".into(),
        hpa_dco: hpa.unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_nvme_ioctl_constant_matches_known_value() {
        use super::secure_erase::secure_erase_linux::{nvme_admin_cmd, NVME_IOCTL_ADMIN_CMD};
        assert_eq!(std::mem::size_of::<nvme_admin_cmd>(), 72);
        // upstream value for x86_64: 0xC0484E41
        assert_eq!(NVME_IOCTL_ADMIN_CMD, 0xC048_4E41);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn nvme_admin_command_opcode_encoding() {
        use super::secure_erase::secure_erase_linux::nvme_admin_cmd;
        let cmd = nvme_admin_cmd::new(0x80, 0xFFFF_FFFF, 300_000);
        assert_eq!(cmd.opcode, 0x80);
        assert_eq!(cmd.nsid, 0xFFFF_FFFF);
        assert_eq!(cmd.data_len, 0);
    }

    #[test]
    fn flash_media_is_refused_without_hardware_command() {
        let err = hardware_erase("/dev/sda", MediaType::UsbFlash).unwrap_err();
        assert!(matches!(err, HardwareEraseError::Unsupported(_)));
    }

    #[test]
    fn vendored_password_has_fixed_width_for_ata_sector() {
        #[cfg(windows)]
        {
            assert_eq!(ATA_SECURITY_ERASE_PASSWORD.len(), 32);
        }
        #[cfg(target_os = "linux")]
        {
            let password: Vec<u8> = "PURGENT-ATA-ERASE-2026-SANITIZE"
                .bytes()
                .chain([0u8; 32])
                .take(32)
                .collect();
            assert_eq!(password.len(), 32);
            assert_eq!(password[0], b'P');
        }
    }

    #[cfg(windows)]
    #[test]
    fn ata_flags_matches_wdk_constants() {
        use crate::modules::storage::hpa_dco::{ATA_FLAGS_DATA_IN, ATA_FLAGS_DATA_OUT};
        assert_eq!(
            ATA_FLAGS_DATA_OUT, 0x04,
            "ATA_FLAGS_DATA_OUT must be 0x04 per ntddscsi.h"
        );
        assert_eq!(
            ATA_FLAGS_DATA_IN, 0x02,
            "ATA_FLAGS_DATA_IN must be 0x02 per ntddscsi.h"
        );
        assert_eq!(ata_flags(true), 0x04, "data-out command must set DATA_OUT");
        assert_eq!(
            ata_flags(false),
            0x00,
            "no-data command must leave flags at zero"
        );
    }

    #[cfg(windows)]
    #[test]
    fn ata_task_header_serializes_wdk_data_out_flag() {
        use crate::modules::storage::hpa_dco::{serialize_apt, AtaPassThroughEx};
        let header_size = size_of::<AtaPassThroughEx>();
        let mut buf = vec![0u8; header_size];
        let apt = AtaPassThroughEx {
            Length: header_size as u16,
            AtaFlags: ata_flags(true),
            PathId: 0,
            TargetId: 0,
            Lun: 0,
            ReservedAsUchar: 0,
            DataTransferLength: 512,
            TimeOutValue: 30,
            ReservedAsUlong: 0,
            DataBufferOffset: header_size as u32,
            PreviousTaskFile: [0; 8],
            CurrentTaskFile: [0; 8],
        };
        serialize_apt(&apt, &mut buf);
        // AtaFlags occupies bytes 2..4 of the serialized ATA_PASS_THROUGH_EX structure
        assert_eq!(
            buf[2..4],
            0x0004u16.to_le_bytes(),
            "serialized header must carry WDK ATA_FLAGS_DATA_OUT (0x04) at byte offset 2..4"
        );
    }
}
