//! HPA / DCO (Host Protected Area / Device Configuration Overlay) detection and removal.
//!
//! Data can be hidden in a Host Protected Area or behind a Device Configuration Overlay
//! and survive a "complete" wipe. This module parses the ATA IDENTIFY response into a
//! pure, cross-platform `HpaDcoState` (unit-tested everywhere), encodes the ATA task
//! files needed to restore the native capacity (SET MAX ADDRESS / SET MAX ADDRESS EXT)
//! and to remove a DCO (SET DEVICE CONFIGURATION RESET), and provides OS-specific
//! query/removal behind `#[cfg]` gates that are only executed against real hardware.

#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct HpaDcoState {
    pub hpa_present: bool,
    pub dco_present: bool,
    pub removable: bool,
    pub max_current_lba: u64,
    pub max_native_lba: u64,
}

impl HpaDcoState {
    pub fn label(&self) -> String {
        if self.hpa_present || self.dco_present {
            let mut parts = Vec::new();
            if self.hpa_present {
                parts.push("HPA detected".to_string());
            }
            if self.dco_present {
                parts.push("DCO present".to_string());
            }
            parts.join("; ")
        } else {
            "none present".to_string()
        }
    }
}

pub const IDENTIFY_LBA28_CAP_WORD: usize = 60;
pub const IDENTIFY_LBA48_MAX_WORD: usize = 100;
// ATA IDENTIFY word 162 bit 0 signals "Device Configuration Overlay" support (word 163
// holds the actual DCO bits when the feature set is supported).
pub const IDENTIFY_DCO_FEATURE_WORD: usize = 162;
pub const IDENTIFY_REMOVABLE_WORD: usize = 0;

/// Pure parser for a raw ATA IDENTIFY DEVICE / IDENTIFY DEVICE DMA response (512 bytes).
///
/// - `max_current_lba` reflects the currently addressable capacity (words 60/61).
/// - `max_native_lba` reflects the native maximum (words 100-103 for LBA48; falls
///   back to the LBA28 value when the 48-bit field is empty), saturation-capped to
///   `0x0000_FFFF_FFFF_FFFF` sectors.
/// - HPA is present when the current capacity is non-zero and smaller than native.
/// - DCO is flagged from the ATA feature-set tag in word 162 bit 0.
pub fn parse_identify(data: &[u8; 512]) -> HpaDcoState {
    let word = |i: usize| -> u16 { u16::from_le_bytes([data[i * 2], data[i * 2 + 1]]) };

    let lba28_cap =
        ((word(IDENTIFY_LBA28_CAP_WORD + 1) as u64) << 16) | word(IDENTIFY_LBA28_CAP_WORD) as u64;
    let lba48_raw = ((word(IDENTIFY_LBA48_MAX_WORD + 3) as u64) << 48)
        | ((word(IDENTIFY_LBA48_MAX_WORD + 2) as u64) << 32)
        | ((word(IDENTIFY_LBA48_MAX_WORD + 1) as u64) << 16)
        | word(IDENTIFY_LBA48_MAX_WORD) as u64;
    const LBA48_SECTOR_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;

    let current = lba28_cap;
    // When LBA48 is present, words 100-103 report the *native* maximum LBA
    // (0-indexed), so the sector count is raw + 1, capped to the LBA48 address
    // space. Otherwise fall back to the LBA28 value.
    let native = if lba48_raw > 0 {
        lba48_raw.saturating_add(1).min(LBA48_SECTOR_MASK)
    } else {
        current
    };

    let dco_feature_bits = word(IDENTIFY_DCO_FEATURE_WORD);
    HpaDcoState {
        hpa_present: current > 0 && current < native,
        dco_present: dco_feature_bits & 0x0001 != 0,
        removable: word(IDENTIFY_REMOVABLE_WORD) & 0x0004 != 0,
        max_current_lba: current,
        max_native_lba: native,
    }
}

// ---------------------------------------------------------------------------
// ATA task-file encodings for HPA/DCO removal.
//
// Pure, cross-platform byte builders. The returned `AtaTaskFile` carries the low
// 8-byte register image plus an optional 8-byte "previous" (high) block used by
// 48-bit commands (SET MAX ADDRESS EXT). OS backends below simply ship these bytes
// through their passthrough ioctl; nothing here touches hardware.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtaTaskFile {
    /// Low register block (8 bytes). Register order matches ATA_PASS_THROUGH_EX /
    /// SAT: features, sector_count, lba_low, lba_mid, lba_high, device, status, command.
    pub current: [u8; 8],
    /// High register block used by 48-bit commands (SET MAX ADDRESS EXT).
    /// Emptied (all zero) for 28-bit commands.
    pub previous: [u8; 8],
    /// True when the 48-bit command block must be used (SET MAX ADDRESS EXT).
    pub lba48: bool,
}

/// SET MAX ADDRESS (28-bit, opcode 0xF9) with `max_lba` as the highest
/// addressable sector. Feature = 0x00 (set max address, no lock).
pub fn set_max_address_lba28(max_lba: u32) -> AtaTaskFile {
    AtaTaskFile {
        current: [
            0x00, // features: SET MAX ADDRESS
            0x00, // sector count
            (max_lba & 0xFF) as u8,
            ((max_lba >> 8) & 0xFF) as u8,
            ((max_lba >> 16) & 0xFF) as u8,
            0x40 | (((max_lba >> 24) & 0x0F) as u8), // device: L=1 + top nibble
            0x00,                                    // status (out)
            0xF9,                                    // command: SET MAX ADDRESS
        ],
        previous: [0u8; 8],
        lba48: false,
    }
}

/// SET MAX ADDRESS EXT (48-bit, opcode 0x37) with `max_lba` as the highest
/// addressable sector. Feature = 0x00 (set max address, no lock).
pub fn set_max_address_lba48(max_lba: u64) -> AtaTaskFile {
    AtaTaskFile {
        current: [
            0x00, // features (low)
            0x00, // sector count (low)
            (max_lba & 0xFF) as u8,
            ((max_lba >> 8) & 0xFF) as u8,
            ((max_lba >> 16) & 0xFF) as u8,
            0x40, // device: L bit for 48-bit; upper bits are in `previous`
            0x00, // status (out)
            0x37, // command: SET MAX ADDRESS EXT
        ],
        previous: [
            0x00,                           // features (high)
            0x00,                           // sector count (high)
            ((max_lba >> 24) & 0xFF) as u8, // LBA low (high)
            ((max_lba >> 32) & 0xFF) as u8,
            ((max_lba >> 40) & 0xFF) as u8,
            0x00,
            0x00,
            0x00,
        ],
        lba48: true,
    }
}

/// SET DEVICE CONFIGURATION RESET (opcode 0xB1, feature 0x04). Restores the
/// device to its factory feature-set configuration, removing a previously
/// applied Device Configuration Overlay. This is the standard "DCO reset"
/// subcommand and takes no data.
pub fn device_configuration_reset() -> AtaTaskFile {
    AtaTaskFile {
        current: [
            0x04, // features (low): 0x04 = Set Device Configuration Reset
            0x00, // sector count (low)
            0x00, 0x00, 0x00, 0x00, 0x00, // status (out)
            0xB1, // command: SET DEVICE CONFIGURATION
        ],
        previous: [0u8; 8],
        lba48: false,
    }
}

/// Outcome of an HPA/DCO removal attempt; surfaced in wipe reports.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct HpaDcoRemoval {
    pub attempted: bool,
    pub hpa_clear_sent: bool,
    pub dco_reset_sent: bool,
    pub removed: bool,
    pub detail: String,
}

impl Default for HpaDcoRemoval {
    fn default() -> Self {
        HpaDcoRemoval {
            attempted: false,
            hpa_clear_sent: false,
            dco_reset_sent: false,
            removed: false,
            detail: String::new(),
        }
    }
}

impl AtaTaskFile {
    /// True when this task file issues SET MAX ADDRESS (28-bit) or
    /// SET MAX ADDRESS EXT (48-bit); i.e. it restores the native HPA capacity.
    pub fn is_set_max(&self) -> bool {
        self.current[7] == 0xF9 || self.current[7] == 0x37
    }

    /// True when this task file issues SET DEVICE CONFIGURATION (DCO reset);
    /// bit 4 (0x04) of the feature register selects the "reset" subcommand.
    pub fn is_dco_reset(&self) -> bool {
        self.current[7] == 0xB1 && self.current[0] == 0x04
    }
}

/// Plan the ATA task files needed to fully expose the native capacity of `state`.
///
/// - HPA present: one SET MAX ADDRESS on `max_native_lba - 1`; the 28-bit form is
///   used when the native LBA fits, otherwise the 48-bit form.
/// - DCO present: one SET DEVICE CONFIGURATION RESET.
/// - Order: HPA restore first, then DCO reset (a DCO reset can drop additional
///   feature sets but never re-triggers an HPA).
/// - Nothing planned when both are clean.
pub fn plan_removal(state: &HpaDcoState) -> Vec<AtaTaskFile> {
    let mut cmds = Vec::new();
    if state.hpa_present && state.max_native_lba > 0 {
        let max_lba = state.max_native_lba - 1;
        if max_lba <= 0x0FFF_FFFF {
            cmds.push(set_max_address_lba28(max_lba as u32));
        } else {
            cmds.push(set_max_address_lba48(max_lba));
        }
    }
    if state.dco_present {
        cmds.push(device_configuration_reset());
    }
    cmds
}

#[cfg(windows)]
pub(crate) fn query_hpa_dco(path: &str) -> Option<HpaDcoState> {
    use std::fs::File;
    use std::os::windows::io::FromRawHandle;

    let wide: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
    let handle = unsafe {
        windows_sys::Win32::Storage::FileSystem::CreateFileW(
            wide.as_ptr(),
            windows_sys::Win32::Foundation::GENERIC_READ,
            windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ
                | windows_sys::Win32::Storage::FileSystem::FILE_SHARE_WRITE,
            std::ptr::null(),
            windows_sys::Win32::Storage::FileSystem::OPEN_EXISTING,
            windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL,
            std::ptr::null_mut(),
        )
    };
    if handle == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
        return None;
    }
    let file = unsafe { File::from_raw_handle(handle as *mut _) };
    let identify = ata_identify(&file);
    drop(file);
    identify.map(|buf| parse_identify(&buf))
}

#[cfg(windows)]
fn ata_identify(file: &std::fs::File) -> Option<[u8; 512]> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::IO::DeviceIoControl;

    let header_size = size_of::<AtaPassThroughEx>();
    let mut buffer = vec![0u8; header_size + 512];
    let header = AtaPassThroughEx {
        Length: header_size as u16,
        AtaFlags: ATA_FLAGS_DATA_IN,
        PathId: 0,
        TargetId: 0,
        Lun: 0,
        ReservedAsUchar: 0,
        DataTransferLength: 512,
        TimeOutValue: 30_000,
        ReservedAsUlong: 0,
        DataBufferOffset: header_size as u32,
        PreviousTaskFile: [0; 8],
        CurrentTaskFile: [0, 0, 0, 0, 0, 0, 0, 0xEC], // IDENTIFY DEVICE
    };
    serialize_apt(&header, &mut buffer[..header_size]);

    let mut returned = 0u32;
    let ok = unsafe {
        DeviceIoControl(
            file.as_raw_handle(),
            IOCTL_ATA_PASS_THROUGH,
            buffer.as_ptr() as *const _,
            header_size as u32,
            buffer.as_mut_ptr() as *mut _,
            buffer.len() as u32,
            &mut returned,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 || (returned as usize) < header_size + 512 {
        return None;
    }
    let mut out = [0u8; 512];
    out.copy_from_slice(&buffer[header_size..header_size + 512]);
    Some(out)
}

#[cfg(windows)]
pub(crate) const IOCTL_ATA_PASS_THROUGH: u32 =
    (0x0000_0004 << 16) | (0x03 << 14) | (0x040b << 2) | 0;

#[cfg(windows)]
// Windows ATA pass-through flags (WDK ntddscsi.h): DRDY_REQUIRED=0x01,
// DATA_IN=0x02, DATA_OUT=0x04, 48BIT_COMMAND=0x08, USE_DMA=0x10, NO_MULTIPLE=0x20.
// Note: the value really is 0x02 for DATA_IN (0x20 is NO_MULTIPLE).
const ATA_FLAGS_DATA_IN: u16 = 0x02;
#[cfg(windows)]
const ATA_FLAGS_48BIT_COMMAND: u16 = 0x08;

#[cfg(windows)]
#[allow(non_snake_case)]
#[repr(C)]
pub(crate) struct AtaPassThroughEx {
    pub Length: u16,
    pub AtaFlags: u16,
    pub PathId: u8,
    pub TargetId: u8,
    pub Lun: u8,
    pub ReservedAsUchar: u8,
    pub DataTransferLength: u32,
    pub TimeOutValue: u32,
    pub ReservedAsUlong: u32,
    pub DataBufferOffset: u32,
    pub PreviousTaskFile: [u8; 8],
    pub CurrentTaskFile: [u8; 8],
}

#[cfg(windows)]
pub(crate) fn serialize_apt(apt: &AtaPassThroughEx, out: &mut [u8]) {
    let mut off = 0usize;
    out[off..off + 2].copy_from_slice(&apt.Length.to_le_bytes());
    off += 2;
    out[off..off + 2].copy_from_slice(&apt.AtaFlags.to_le_bytes());
    off += 2;
    out[off] = apt.PathId;
    off += 1;
    out[off] = apt.TargetId;
    off += 1;
    out[off] = apt.Lun;
    off += 1;
    out[off] = apt.ReservedAsUchar;
    off += 1;
    out[off..off + 4].copy_from_slice(&apt.DataTransferLength.to_le_bytes());
    off += 4;
    out[off..off + 4].copy_from_slice(&apt.TimeOutValue.to_le_bytes());
    off += 4;
    out[off..off + 4].copy_from_slice(&apt.ReservedAsUlong.to_le_bytes());
    off += 4;
    out[off..off + 4].copy_from_slice(&apt.DataBufferOffset.to_le_bytes());
    off += 4;
    for i in 0..8 {
        out[off + i] = apt.PreviousTaskFile[i];
    }
    off += 8;
    for i in 0..8 {
        out[off + i] = apt.CurrentTaskFile[i];
    }
}

#[cfg(windows)]
/// Remove a detected HPA/DCO via ATA_PASS_THROUGH on a physical disk handle.
///
/// Returns a `HpaDcoRemoval` describing what was sent and whether a follow-up
/// IDENTIFY shows the hidden area is gone. No data is transferred by any of the
/// commands issued here (SET MAX ADDRESS / SET MAX ADDRESS EXT / DCO RESET).
pub(crate) fn remove_hpa_dco(path: &str) -> Result<HpaDcoRemoval, String> {
    use std::fs::File;
    use std::os::windows::io::FromRawHandle;

    let wide: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
    let handle = unsafe {
        windows_sys::Win32::Storage::FileSystem::CreateFileW(
            wide.as_ptr(),
            windows_sys::Win32::Foundation::GENERIC_READ
                | windows_sys::Win32::Foundation::GENERIC_WRITE,
            windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ
                | windows_sys::Win32::Storage::FileSystem::FILE_SHARE_WRITE,
            std::ptr::null(),
            windows_sys::Win32::Storage::FileSystem::OPEN_EXISTING,
            windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL,
            std::ptr::null_mut(),
        )
    };
    if handle == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
        return Err(format!(
            "cannot open ATA device {path} for HPA/DCO removal (run as administrator)"
        ));
    }
    let file = unsafe { File::from_raw_handle(handle as *mut _) };

    let state = ata_identify(&file)
        .map(|buf| parse_identify(&buf))
        .ok_or_else(|| "cannot query HPA/DCO state before removal".to_string())?;
    let mut removal = HpaDcoRemoval {
        attempted: true,
        ..HpaDcoRemoval::default()
    };
    let cmds = plan_removal(&state);
    if cmds.is_empty() {
        removal.detail = "no HPA or DCO present".to_string();
        return Ok(removal);
    }
    for task in &cmds {
        if task.is_set_max() {
            removal.hpa_clear_sent = true;
        } else if task.is_dco_reset() {
            removal.dco_reset_sent = true;
        }
        send_ata_passthrough(&file, task).map_err(|err| {
            format!(
                "taskfile command {:#04x} failed for {path}: {err}",
                task.current[7]
            )
        })?;
    }
    let after = ata_identify(&file)
        .map(|buf| parse_identify(&buf))
        .unwrap_or_default();
    removal.removed =
        (!state.hpa_present || !after.hpa_present) && (!state.dco_present || !after.dco_present);
    removal.detail = if removal.removed {
        "HPA/DCO removal issued; re-query shows native capacity restored".to_string()
    } else {
        "HPA/DCO commands issued, but re-query still reports hidden capacity".to_string()
    };
    Ok(removal)
}

#[cfg(windows)]
fn send_ata_passthrough(file: &std::fs::File, task: &AtaTaskFile) -> Result<u8, std::io::Error> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::IO::DeviceIoControl;

    let header_size = size_of::<AtaPassThroughEx>();
    let mut buffer = vec![0u8; header_size];
    let header = AtaPassThroughEx {
        Length: header_size as u16,
        AtaFlags: if task.lba48 {
            ATA_FLAGS_48BIT_COMMAND
        } else {
            0
        },
        PathId: 0,
        TargetId: 0,
        Lun: 0,
        ReservedAsUchar: 0,
        DataTransferLength: 0,
        TimeOutValue: 30_000,
        ReservedAsUlong: 0,
        DataBufferOffset: header_size as u32,
        PreviousTaskFile: task.previous,
        CurrentTaskFile: task.current,
    };
    serialize_apt(&header, &mut buffer);

    let mut returned = 0u32;
    let ok = unsafe {
        DeviceIoControl(
            file.as_raw_handle(),
            IOCTL_ATA_PASS_THROUGH,
            buffer.as_ptr() as *const _,
            header_size as u32,
            buffer.as_mut_ptr() as *mut _,
            header_size as u32,
            &mut returned,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(std::io::Error::last_os_error());
    }
    if (returned as usize) < header_size {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "no taskfile registers returned by ATA pass-through",
        ));
    }
    // CurrentTaskFile[6] carries the device status register at byte header_size - 2.
    let status = buffer[header_size - 2];
    if status & 0x01 != 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!(
                "ATA taskfile command {:#04x} reported error status 0x{status:02X}",
                task.current[7]
            ),
        ));
    }
    Ok(status)
}

#[cfg(target_os = "linux")]
// The `libc` crate does not expose the HDIO_* ioctls; these values come from
// linux/`include/uapi/linux/hdreg.h`.
const HDIO_GET_IDENTITY: libc::c_ulong = 0x030d;
#[cfg(target_os = "linux")]
const HDIO_DRIVE_TASKFILE: libc::c_ulong = 0x031d;

#[cfg(target_os = "linux")]
pub(crate) fn query_hpa_dco(path: &str) -> Option<HpaDcoState> {
    use std::os::unix::fs::OpenOptionsExt;
    use std::os::unix::io::AsRawFd;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
        .ok()?;
    let mut identify = [0u8; 512];
    let ret = unsafe { libc::ioctl(file.as_raw_fd(), HDIO_GET_IDENTITY, &mut identify) };
    if ret != 0 {
        return None;
    }
    Some(parse_identify(&identify))
}

#[cfg(target_os = "linux")]
/// Remove a detected HPA/DCO via the HDIO_DRIVE_TASKFILE ioctl on a raw disk.
///
/// Mirrors the Windows backend: plan, issue each non-data task file, then
/// re-run IDENTIFY to confirm the native capacity is visible again.
pub(crate) fn remove_hpa_dco(path: &str) -> Result<HpaDcoRemoval, String> {
    let file = open_raw_disk(path)?;
    let state =
        query_hpa_dco(path).ok_or_else(|| format!("cannot query HPA/DCO state for {path}"))?;
    let mut removal = HpaDcoRemoval {
        attempted: true,
        ..HpaDcoRemoval::default()
    };
    let cmds = plan_removal(&state);
    if cmds.is_empty() {
        removal.detail = "no HPA or DCO present".to_string();
        return Ok(removal);
    }
    for task in &cmds {
        if task.is_set_max() {
            removal.hpa_clear_sent = true;
        } else if task.is_dco_reset() {
            removal.dco_reset_sent = true;
        }
        send_ide_taskfile(&file, task).map_err(|err| {
            format!(
                "taskfile command {:#04x} failed for {path}: {err}",
                task.current[7]
            )
        })?;
    }
    let after = query_hpa_dco(path).unwrap_or_default();
    removal.removed =
        (!state.hpa_present || !after.hpa_present) && (!state.dco_present || !after.dco_present);
    removal.detail = if removal.removed {
        "HPA/DCO removal issued; re-query shows native capacity restored".to_string()
    } else {
        "HPA/DCO commands issued, but re-query still reports hidden capacity".to_string()
    };
    Ok(removal)
}

#[cfg(target_os = "linux")]
fn open_raw_disk(path: &str) -> Result<std::fs::File, String> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
        .map_err(|err| format!("cannot open raw disk {path}: {err} (root required)"))
}

#[cfg(target_os = "linux")]
fn send_ide_taskfile(file: &std::fs::File, task: &AtaTaskFile) -> Result<(), String> {
    use std::os::unix::io::AsRawFd;

    let mut request: IdeTaskRequest = unsafe { std::mem::zeroed() };
    request.io_ports[1] = task.current[0]; // features
    request.io_ports[2] = task.current[1]; // sector count
    request.io_ports[3] = task.current[2]; // LBA low
    request.io_ports[4] = task.current[3]; // LBA mid
    request.io_ports[5] = task.current[4]; // LBA high
    request.io_ports[6] = task.current[5]; // device (L bit / 28-bit top nibble)
    request.io_ports[7] = task.current[7]; // command (status register is output-only)

    if task.lba48 {
        request.hob_ports[1] = task.previous[0]; // features (high)
        request.hob_ports[2] = task.previous[1]; // sector count (high)
        request.hob_ports[3] = task.previous[2]; // LBA 24..31
        request.hob_ports[4] = task.previous[3]; // LBA 32..39
        request.hob_ports[5] = task.previous[4]; // LBA 40..47
    }

    // For 28-bit commands use the per-bit path so the kernel keeps our device
    // register (28-bit LBA nibble included) instead of masking it with 0xE0.
    // For 48-bit commands select the HOB registers as well.
    request.out_flags.all = if task.lba48 {
        // feature|sector|lcyl|hcyl|status_command + feature_hob|sector_hob|lcyl_hob|hcyl_hob
        0x00B6 | 0x3600
    } else {
        // feature|sector|lcyl|hcyl|status_command (device register is loaded regardless)
        0x00B6
    };

    // No data is transferred: data_phase = TASKFILE_NO_DATA (0) and
    // req_cmd = IDE_DRIVE_TASK_NO_DATA (0), both already zeroed above.
    let rc = unsafe { libc::ioctl(file.as_raw_fd(), HDIO_DRIVE_TASKFILE, &mut request) };
    if rc < 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    Ok(())
}

// Mirror of `ide_task_request_t` / `ide_task_request_s` from
// linux/`include/uapi/linux/hdreg.h`. Note: `error`/`status` registers are
// output-only, so io_ports[0] and io_ports[6] stay zero here.
#[cfg(target_os = "linux")]
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct IdeRegValid {
    pub all: u16,
}

#[cfg(target_os = "linux")]
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct IdeTaskRequest {
    pub io_ports: [u8; 8],
    pub hob_ports: [u8; 8],
    pub out_flags: IdeRegValid,
    pub in_flags: IdeRegValid,
    pub data_phase: i32,
    pub req_cmd: i32,
    pub out_size: libc::c_ulong,
    pub in_size: libc::c_ulong,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identify_with_words(words: &[(usize, u16)]) -> [u8; 512] {
        let mut data = [0u8; 512];
        for &(idx, w) in words {
            data[idx * 2] = (w & 0xFF) as u8;
            data[idx * 2 + 1] = (w >> 8) as u8;
        }
        data
    }

    #[test]
    fn dco_feature_word_rules_flag_maintenance() {
        let clean = parse_identify(&[0u8; 512]);
        assert!(!clean.hpa_present);
        assert!(!clean.dco_present);
        assert_eq!(clean.max_native_lba, 0);
        assert_eq!(clean.max_current_lba, 0);
    }

    #[test]
    fn hpa_detected_when_current_smaller_than_native() {
        // Native max LBA48 = 1000 sectors; current (LB28 words) = 500 sectors => HPA.
        let data = identify_with_words(&[
            (IDENTIFY_LBA48_MAX_WORD, 1000 & 0xFFFF),
            (IDENTIFY_LBA48_MAX_WORD + 1, (1000 >> 16) as u16),
            (IDENTIFY_LBA48_MAX_WORD + 2, 0),
            (IDENTIFY_LBA48_MAX_WORD + 3, 0),
            (IDENTIFY_LBA28_CAP_WORD, 500 & 0xFFFF),
            (IDENTIFY_LBA28_CAP_WORD + 1, 0),
        ]);
        let state = parse_identify(&data);
        assert!(state.hpa_present, "native>current must flag HPA");
        assert_eq!(state.max_current_lba, 500);
        assert_eq!(state.max_native_lba, 1001);
    }

    #[test]
    fn no_hpa_when_capacities_match() {
        // Native (LBA48, 0-indexed max LBA = 4095) and current (LBA28 = 4096 sectors)
        // describe the same capacity, so no HPA is flagged.
        let data = identify_with_words(&[
            (IDENTIFY_LBA48_MAX_WORD, 4095 & 0xFFFF),
            (IDENTIFY_LBA48_MAX_WORD + 1, (4095 >> 16) as u16),
            (IDENTIFY_LBA48_MAX_WORD + 2, 0),
            (IDENTIFY_LBA48_MAX_WORD + 3, 0),
            (IDENTIFY_LBA28_CAP_WORD, 4096 & 0xFFFF),
            (IDENTIFY_LBA28_CAP_WORD + 1, (4096 >> 16) as u16),
        ]);
        assert!(!parse_identify(&data).hpa_present);
    }

    #[test]
    fn dco_flag_taken_from_word_162_bit0() {
        let with_dco = identify_with_words(&[(IDENTIFY_DCO_FEATURE_WORD, 0x0001)]);
        assert!(parse_identify(&with_dco).dco_present);
        let without_dco = identify_with_words(&[(IDENTIFY_DCO_FEATURE_WORD, 0x0002)]);
        assert!(!parse_identify(&without_dco).dco_present);
    }

    #[test]
    fn removable_flag_taken_from_word0_bit2() {
        let removable = identify_with_words(&[(0, 0x8004)]);
        assert!(parse_identify(&removable).removable);
        let fixed = identify_with_words(&[(0, 0x0040)]);
        assert!(!parse_identify(&fixed).removable);
    }

    #[test]
    fn lba48_saturation_cap_applies() {
        // Current addressable capacity is maxed at the LBA28 range; native capacity
        // saturates at the LBA48 ceiling (0xFFFF_FFFF_FFFF sectors).
        let data = identify_with_words(&[
            (IDENTIFY_LBA28_CAP_WORD, 0xFFFF),
            (IDENTIFY_LBA28_CAP_WORD + 1, 0xFFFF),
            (IDENTIFY_LBA48_MAX_WORD, 0xFFFF),
            (IDENTIFY_LBA48_MAX_WORD + 1, 0xFFFF),
            (IDENTIFY_LBA48_MAX_WORD + 2, 0xFFFF),
            (IDENTIFY_LBA48_MAX_WORD + 3, 0xFF),
        ]);
        let state = parse_identify(&data);
        assert_eq!(state.max_native_lba, 0x0000_FFFF_FFFF_FFFF);
        assert_eq!(state.max_current_lba, 0xFFFF_FFFF);
        assert!(state.hpa_present);
    }

    #[cfg(windows)]
    #[test]
    fn ata_passthrough_ioctl_constant_is_stable() {
        use super::serialize_apt;
        // CTL_CODE(IOCTL_SCSI_BASE=0x4, 0x040b, METHOD_BUFFERED, R|W) == 0x4D02C
        assert_eq!(super::IOCTL_ATA_PASS_THROUGH, 0x0004_D02C);
        let apt = AtaPassThroughEx {
            Length: 40,
            AtaFlags: ATA_FLAGS_DATA_IN,
            PathId: 0,
            TargetId: 0,
            Lun: 0,
            ReservedAsUchar: 0,
            DataTransferLength: 512,
            TimeOutValue: 30_000,
            ReservedAsUlong: 0,
            DataBufferOffset: 40,
            PreviousTaskFile: [0; 8],
            CurrentTaskFile: [0, 0, 0, 0, 0, 0, 0, 0xEC],
        };
        let mut buf = [0u8; 40];
        serialize_apt(&apt, &mut buf);
        assert_eq!(&buf[0..2], &40u16.to_le_bytes());
        assert_eq!(&buf[2..4], &ATA_FLAGS_DATA_IN.to_le_bytes());
        assert_eq!(&buf[20..24], &40u32.to_le_bytes());
        assert_eq!(&buf[32..40], &[0, 0, 0, 0, 0, 0, 0, 0xEC]);
    }

    #[cfg(windows)]
    #[test]
    fn data_in_flag_value_matches_wdk() {
        // WDK ntddscsi.h: DATA_IN = 1 << 1 = 0x02 (0x20 is NO_MULTIPLE).
        assert_eq!(ATA_FLAGS_DATA_IN, 0x0002);
        assert_eq!(ATA_FLAGS_48BIT_COMMAND, 0x0008);
    }

    #[test]
    fn set_max_address_lba28_encodes_registers() {
        let task = set_max_address_lba28(0x0ABC_DEFF);
        assert_eq!(
            task.current[..7],
            [0x00, 0x00, 0xFF, 0xDE, 0xBC, 0x4A, 0x00],
            "features=0, sector count=0, LBA little-endian, device=0x40|top nibble"
        );
        assert_eq!(task.current[7], 0xF9);
        assert_eq!(task.previous, [0u8; 8]);
        assert!(!task.lba48);
        assert!(task.is_set_max());
        assert!(!task.is_dco_reset());
    }

    #[test]
    fn set_max_address_lba28_keeps_lba_bits_in_device_register() {
        // 28-bit SET MAX carries LBA(27:24) in device bits 3..0.
        let task = set_max_address_lba28(0x0000_0000);
        assert_eq!(task.current[5], 0x40);
        let task = set_max_address_lba28(0x0FFF_FFFF);
        assert_eq!(task.current[5], 0x40 | 0x0F);
    }

    #[test]
    fn set_max_address_lba48_populates_hob_registers() {
        let task = set_max_address_lba48(0x0123_4567_89AB);
        assert_eq!(
            task.current[..7],
            [0x00, 0x00, 0xAB, 0x89, 0x67, 0x40, 0x00],
            "low block: features=0, count=0, LBA(23:0), device=0x40"
        );
        assert_eq!(task.current[7], 0x37);
        assert_eq!(task.previous[..5], [0x00, 0x00, 0x45, 0x23, 0x01]);
        assert!(task.lba48);
        assert!(task.is_set_max());
    }

    #[test]
    fn device_configuration_reset_encodes_feature_04() {
        let task = device_configuration_reset();
        assert_eq!(task.current[0], 0x04, "feature 0x04 selects DCO reset");
        assert_eq!(task.current[7], 0xB1);
        assert_eq!(task.current[1..7], [0, 0, 0, 0, 0, 0]);
        assert!(!task.lba48);
        assert!(task.is_dco_reset());
        assert!(!task.is_set_max());
    }

    #[test]
    fn plan_removal_restores_hpa_with_set_max() {
        let state = HpaDcoState {
            hpa_present: true,
            dco_present: false,
            removable: true,
            max_current_lba: 500,
            max_native_lba: 1001,
        };
        let cmds = plan_removal(&state);
        assert_eq!(cmds.len(), 1);
        assert!(cmds[0].is_set_max());
        // SET MAX on the highest native LBA (native sectors - 1), 28-bit fit:
        // max_lba = 1000 = 0x3E8, little-endian in current[2..5].
        assert_eq!(cmds[0].current[..5], [0x00, 0x00, 0xE8, 0x03, 0x00]);
        assert!(!cmds[0].lba48);
    }

    #[test]
    fn plan_removal_uses_lba48_for_large_native_capacity() {
        let big: u64 = 0xAB000000; // > LBA28 limit (0x0FFF_FFFF)
        let state = HpaDcoState {
            hpa_present: true,
            dco_present: false,
            removable: true,
            max_current_lba: 0xFFFF_FFFF,
            max_native_lba: big + 1,
        };
        let cmds = plan_removal(&state);
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].current[7], 0x37, "SET MAX ADDRESS EXT");
        assert!(cmds[0].lba48);
        assert_eq!(cmds[0].previous[2], 0xAB, "LBA 24..31 in HOB register");
        assert_eq!(cmds[0].previous[3], 0x00);
        assert_eq!(cmds[0].previous[4], 0x00);
    }

    #[test]
    fn plan_removal_resets_dco() {
        let state = HpaDcoState {
            hpa_present: false,
            dco_present: true,
            removable: true,
            max_current_lba: 1000,
            max_native_lba: 1001,
        };
        let cmds = plan_removal(&state);
        assert_eq!(cmds.len(), 1);
        assert!(cmds[0].is_dco_reset());
    }

    #[test]
    fn plan_removal_orders_hpa_before_dco() {
        let state = HpaDcoState {
            hpa_present: true,
            dco_present: true,
            removable: true,
            max_current_lba: 500,
            max_native_lba: 1001,
        };
        let cmds = plan_removal(&state);
        assert_eq!(cmds.len(), 2);
        assert!(cmds[0].is_set_max(), "HPA restored before DCO reset");
        assert!(cmds[1].is_dco_reset());
    }

    #[test]
    fn plan_removal_empty_when_clean() {
        let state = HpaDcoState {
            hpa_present: false,
            dco_present: false,
            removable: false,
            max_current_lba: 0,
            max_native_lba: 0,
        };
        assert!(plan_removal(&state).is_empty());
    }

    #[test]
    fn hpa_dco_removal_starts_false_by_default() {
        let removal = HpaDcoRemoval::default();
        assert_eq!(removal.attempted, false);
        assert_eq!(removal.hpa_clear_sent, false);
        assert_eq!(removal.dco_reset_sent, false);
        assert_eq!(removal.removed, false);
        assert_eq!(removal.detail, "");
    }
}
