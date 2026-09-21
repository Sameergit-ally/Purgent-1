use std::mem::{size_of, zeroed};
use std::os::windows::ffi::OsStrExt;
use std::ptr;

use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
    SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW,
    SetupDiGetDeviceInterfaceDetailW, DIGCF_DEVICEINTERFACE, DIGCF_PRESENT,
    SP_DEVICE_INTERFACE_DATA,
};
use windows_sys::Win32::Foundation::{CloseHandle, GENERIC_READ, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    BusTypeAta, BusTypeNvme, BusTypeRAID, BusTypeSas, BusTypeSata, BusTypeScsi, BusTypeSd,
    BusTypeUsb, CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE,
    OPEN_EXISTING, STORAGE_BUS_TYPE,
};
use windows_sys::Win32::System::Ioctl::{
    PropertyStandardQuery, StorageDeviceProperty, StorageDeviceSeekPenaltyProperty, DISK_GEOMETRY,
    GUID_DEVINTERFACE_DISK, IOCTL_DISK_GET_DRIVE_GEOMETRY_EX, IOCTL_STORAGE_GET_DEVICE_NUMBER,
    IOCTL_STORAGE_QUERY_PROPERTY, STORAGE_PROPERTY_QUERY,
};
use windows_sys::Win32::System::IO::DeviceIoControl;

use super::{BusType, Device, MediaType};

const ACCESS_FALLBACKS: [u32; 2] = [GENERIC_READ, 0x0080];

pub(super) fn list_devices() -> Vec<Device> {
    let mut devices = Vec::new();
    let device_info_set = unsafe {
        SetupDiGetClassDevsW(
            &GUID_DEVINTERFACE_DISK,
            ptr::null(),
            ptr::null_mut(),
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        )
    };
    if device_info_set == -1 {
        return devices;
    }

    let mut index = 0u32;
    loop {
        let mut interface_data: SP_DEVICE_INTERFACE_DATA = unsafe { zeroed() };
        interface_data.cbSize = size_of::<SP_DEVICE_INTERFACE_DATA>() as u32;
        let found = unsafe {
            SetupDiEnumDeviceInterfaces(
                device_info_set,
                ptr::null(),
                &GUID_DEVINTERFACE_DISK,
                index,
                &mut interface_data,
            )
        };
        if found == 0 {
            break;
        }
        index += 1;

        if let Some(device) = device_from_interface(device_info_set, &interface_data) {
            devices.push(device);
        }
    }

    unsafe { SetupDiDestroyDeviceInfoList(device_info_set) };
    devices.sort_by(|a, b| a.id.cmp(&b.id));
    devices
}

fn device_from_interface(
    device_info_set: isize,
    interface_data: &SP_DEVICE_INTERFACE_DATA,
) -> Option<Device> {
    let mut required = 0u32;
    let _ = unsafe {
        SetupDiGetDeviceInterfaceDetailW(
            device_info_set,
            interface_data,
            ptr::null_mut(),
            0,
            &mut required,
            ptr::null_mut(),
        )
    };
    if required < 8 {
        return None;
    }

    let cb_size = if size_of::<usize>() == 8 { 8u32 } else { 6u32 };
    let mut detail = vec![0u8; required as usize];
    detail[0..4].copy_from_slice(&cb_size.to_ne_bytes());
    let fetched = unsafe {
        SetupDiGetDeviceInterfaceDetailW(
            device_info_set,
            interface_data,
            detail.as_mut_ptr() as *mut _,
            required,
            ptr::null_mut(),
            ptr::null_mut(),
        )
    };
    if fetched == 0 {
        return None;
    }

    let path = read_wide_until_nul(&detail[4..])?;
    if path.is_empty() {
        return None;
    }

    let mut id = path.clone();
    let probe = open_device(&path).and_then(|handle| {
        if let Some(number) = query_device_number(handle) {
            id = format!(r"\\.\PHYSICALDRIVE{}", number);
        }
        let result = probe_device(handle);
        unsafe { CloseHandle(handle) };
        result
    });

    let (model, serial, capacity_bytes, media_type, bus_type, is_removable) = match probe {
        Some(info) => (
            info.model,
            info.serial,
            info.capacity_bytes,
            info.media_type,
            info.bus_type,
            info.is_removable,
        ),
        None => (
            String::new(),
            String::new(),
            0,
            MediaType::Unknown,
            BusType::Other(0),
            false,
        ),
    };

    Some(Device {
        id,
        model,
        serial,
        capacity_bytes,
        media_type,
        bus_type,
        is_removable,
    })
}

fn open_device(path: &str) -> Option<HANDLE> {
    let wide: Vec<u16> = widestr(path);
    for access in ACCESS_FALLBACKS {
        let handle = unsafe {
            CreateFileW(
                wide.as_ptr(),
                access,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                ptr::null(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                ptr::null_mut(),
            )
        };
        if handle != INVALID_HANDLE_VALUE {
            return Some(handle);
        }
    }
    None
}

struct Probe {
    model: String,
    serial: String,
    capacity_bytes: u64,
    media_type: MediaType,
    bus_type: BusType,
    is_removable: bool,
}

fn query_device_number(handle: HANDLE) -> Option<u32> {
    let mut buffer = [0u8; 12];
    let mut returned = 0u32;
    let ok = unsafe {
        DeviceIoControl(
            handle,
            IOCTL_STORAGE_GET_DEVICE_NUMBER,
            ptr::null(),
            0,
            buffer.as_mut_ptr() as *mut _,
            buffer.len() as u32,
            &mut returned,
            ptr::null_mut(),
        )
    };
    if ok == 0 || (returned as usize) < 12 {
        return None;
    }
    buffer[4..8].try_into().ok().map(u32::from_ne_bytes)
}

fn probe_device(handle: HANDLE) -> Option<Probe> {
    let descriptor = query_storage_descriptor(handle)?;
    let vendor = buffer_string(&descriptor.buffer, descriptor.vendor_offset);
    let product = buffer_string(&descriptor.buffer, descriptor.product_offset);
    let serial = buffer_string(&descriptor.buffer, descriptor.serial_offset);
    let model = match (vendor.as_str(), product.as_str()) {
        ("", "") => String::new(),
        (v, "") => v.to_string(),
        ("", p) => p.to_string(),
        (v, p) => format!("{v} {p}"),
    };

    Some(Probe {
        model,
        serial,
        capacity_bytes: query_capacity(handle),
        media_type: classify(
            descriptor.removable,
            descriptor.bus,
            query_seek_penalty(handle),
        ),
        bus_type: bus_type(descriptor.bus),
        is_removable: descriptor.removable,
    })
}

struct Descriptor {
    buffer: Vec<u8>,
    vendor_offset: u32,
    product_offset: u32,
    serial_offset: u32,
    removable: bool,
    bus: STORAGE_BUS_TYPE,
}

fn query_storage_descriptor(handle: HANDLE) -> Option<Descriptor> {
    let mut buffer = vec![0u8; 512];
    let query = STORAGE_PROPERTY_QUERY {
        PropertyId: StorageDeviceProperty,
        QueryType: PropertyStandardQuery,
        AdditionalParameters: [0],
    };
    let mut returned = 0u32;
    let ok = unsafe {
        DeviceIoControl(
            handle,
            IOCTL_STORAGE_QUERY_PROPERTY,
            &query as *const _ as *const _,
            size_of::<STORAGE_PROPERTY_QUERY>() as u32,
            buffer.as_mut_ptr() as *mut _,
            buffer.len() as u32,
            &mut returned,
            ptr::null_mut(),
        )
    };
    if ok == 0 || (returned as usize) < 36 {
        return None;
    }

    let read_u32 = |off: usize| -> Option<u32> {
        buffer
            .get(off..off + 4)?
            .try_into()
            .ok()
            .map(u32::from_ne_bytes)
    };
    let vendor_offset = read_u32(12)?;
    let product_offset = read_u32(16)?;
    let serial_offset = read_u32(24)?;
    let removable = buffer[10] != 0;
    let bus = read_u32(28)? as STORAGE_BUS_TYPE;

    Some(Descriptor {
        buffer,
        vendor_offset,
        product_offset,
        serial_offset,
        removable,
        bus,
    })
}

fn buffer_string(buffer: &[u8], offset: u32) -> String {
    let start = offset as usize;
    if start == 0 || start >= buffer.len() {
        return String::new();
    }
    let end = buffer[start..]
        .iter()
        .position(|&b| b == 0)
        .map(|p| start + p)
        .unwrap_or(buffer.len());
    String::from_utf8_lossy(&buffer[start..end])
        .trim()
        .to_string()
}

fn query_seek_penalty(handle: HANDLE) -> Option<bool> {
    let mut buffer = [0u8; 64];
    let query = STORAGE_PROPERTY_QUERY {
        PropertyId: StorageDeviceSeekPenaltyProperty,
        QueryType: PropertyStandardQuery,
        AdditionalParameters: [0],
    };
    let mut returned = 0u32;
    let ok = unsafe {
        DeviceIoControl(
            handle,
            IOCTL_STORAGE_QUERY_PROPERTY,
            &query as *const _ as *const _,
            size_of::<STORAGE_PROPERTY_QUERY>() as u32,
            buffer.as_mut_ptr() as *mut _,
            buffer.len() as u32,
            &mut returned,
            ptr::null_mut(),
        )
    };
    if ok == 0 || (returned as usize) < 9 {
        return None;
    }
    Some(buffer[8] != 0)
}

fn query_capacity(handle: HANDLE) -> u64 {
    let mut buffer = vec![0u8; 256];
    let mut returned = 0u32;
    let ok = unsafe {
        DeviceIoControl(
            handle,
            IOCTL_DISK_GET_DRIVE_GEOMETRY_EX,
            ptr::null(),
            0,
            buffer.as_mut_ptr() as *mut _,
            buffer.len() as u32,
            &mut returned,
            ptr::null_mut(),
        )
    };
    let disk_size_offset = size_of::<DISK_GEOMETRY>();
    if ok == 0 || (returned as usize) < disk_size_offset + 8 {
        return 0;
    }
    let bytes: [u8; 8] = buffer[disk_size_offset..disk_size_offset + 8]
        .try_into()
        .unwrap_or([0; 8]);
    u64::from_ne_bytes(bytes)
}

fn classify(removable: bool, bus: STORAGE_BUS_TYPE, seek_penalty: Option<bool>) -> MediaType {
    if bus == BusTypeUsb {
        MediaType::UsbFlash
    } else if bus == BusTypeNvme {
        MediaType::Nvme
    } else if bus == BusTypeSd {
        MediaType::SdFlash
    } else if bus == BusTypeAta || bus == BusTypeSata {
        if removable {
            MediaType::UsbFlash
        } else if seek_penalty == Some(false) {
            MediaType::Ssd
        } else {
            MediaType::Hdd
        }
    } else {
        MediaType::Unknown
    }
}

fn bus_type(bus: STORAGE_BUS_TYPE) -> BusType {
    if bus == BusTypeAta {
        BusType::Ata
    } else if bus == BusTypeSata {
        BusType::Sata
    } else if bus == BusTypeNvme {
        BusType::Nvme
    } else if bus == BusTypeUsb {
        BusType::Usb
    } else if bus == BusTypeSd {
        BusType::Sd
    } else if bus == BusTypeSas {
        BusType::Sas
    } else if bus == BusTypeScsi {
        BusType::Scsi
    } else if bus == BusTypeRAID {
        BusType::Raid
    } else {
        BusType::Other(bus as u32)
    }
}

fn widestr(s: &str) -> Vec<u16> {
    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(Some(0))
        .collect()
}

fn read_wide_until_nul(bytes: &[u8]) -> Option<String> {
    if !bytes.len().is_multiple_of(2) {
        return None;
    }
    let mut units = Vec::new();
    for chunk in bytes.as_chunks::<2>().0 {
        let unit = u16::from_ne_bytes([chunk[0], chunk[1]]);
        if unit == 0 {
            break;
        }
        units.push(unit);
    }
    Some(String::from_utf16_lossy(&units))
}
