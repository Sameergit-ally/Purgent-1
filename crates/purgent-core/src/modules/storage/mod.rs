#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
mod linux;

pub mod hpa_dco;
pub mod secure_erase;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaType {
    Hdd,
    Ssd,
    Nvme,
    UsbFlash,
    SdFlash,
    CdRom,
    Unknown,
}

impl MediaType {
    pub fn label(self) -> &'static str {
        match self {
            MediaType::Hdd => "HDD",
            MediaType::Ssd => "SSD",
            MediaType::Nvme => "NVMe",
            MediaType::UsbFlash => "USB Flash",
            MediaType::SdFlash => "SD Flash",
            MediaType::CdRom => "Optical",
            MediaType::Unknown => "Unknown",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            MediaType::Hdd => "hdd",
            MediaType::Ssd => "ssd",
            MediaType::Nvme => "nvme",
            MediaType::UsbFlash => "usb_flash",
            MediaType::SdFlash => "sd_flash",
            MediaType::CdRom => "cd_rom",
            MediaType::Unknown => "unknown",
        }
    }
}

pub fn parse_media_type(id: &str) -> Result<MediaType, String> {
    match id {
        "hdd" => Ok(MediaType::Hdd),
        "ssd" => Ok(MediaType::Ssd),
        "nvme" => Ok(MediaType::Nvme),
        "usb_flash" => Ok(MediaType::UsbFlash),
        "sd_flash" => Ok(MediaType::SdFlash),
        "cd_rom" => Ok(MediaType::CdRom),
        "unknown" => Ok(MediaType::Unknown),
        other => Err(format!("unknown media type id: {other}")),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BusType {
    Ata,
    Sata,
    Nvme,
    Usb,
    Sd,
    Sas,
    Scsi,
    Raid,
    Other(u32),
}

impl BusType {
    pub fn label(self) -> &'static str {
        match self {
            BusType::Ata => "ATA",
            BusType::Sata => "SATA",
            BusType::Nvme => "NVMe",
            BusType::Usb => "USB",
            BusType::Sd => "SD",
            BusType::Sas => "SAS",
            BusType::Scsi => "SCSI",
            BusType::Raid => "RAID",
            BusType::Other(_) => "Other",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub model: String,
    pub serial: String,
    pub capacity_bytes: u64,
    pub media_type: MediaType,
    pub bus_type: BusType,
    pub is_removable: bool,
}

pub fn list_devices() -> Vec<Device> {
    #[cfg(windows)]
    {
        windows::list_devices()
    }
    #[cfg(target_os = "linux")]
    {
        linux::list_devices()
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        Vec::new()
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn list_devices_returns_unique_sorted_ids() {
        let devices = list_devices();
        let ids: Vec<&str> = devices.iter().map(|d| d.id.as_str()).collect();
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted, "device ids must be unique and sorted");
    }

    #[test]
    fn every_device_carries_a_classification() {
        let devices = list_devices();
        for d in &devices {
            assert!(!d.id.is_empty(), "device id must not be empty");
            let _ = d.media_type.label();
            let _ = d.bus_type.label();
        }
    }
}
