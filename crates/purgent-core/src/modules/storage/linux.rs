use std::path::Path;

use super::{BusType, Device, MediaType};

pub fn list_devices() -> Vec<Device> {
    let mut devices = Vec::new();
    let Ok(entries) = std::fs::read_dir("/sys/class/block") else {
        return devices;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.is_empty()
            || name.starts_with("loop")
            || name.starts_with("ram")
            || name.starts_with("zram")
            || name.starts_with("dm-")
            || name.starts_with("md")
        {
            continue;
        }
        let base = entry.path();
        if is_partition(&base, &name) {
            continue;
        }
        let Some(capacity_bytes) = sector_capacity(&base) else {
            continue;
        };
        let is_removable = removable(&base);
        let media_type = classify_media(&name, is_removable);
        let bus_type = classify_bus(&name);
        let (model, serial) = model_and_serial(&base, &name);
        let id = format!("/dev/{name}");
        devices.push(Device {
            id,
            model,
            serial,
            capacity_bytes,
            media_type,
            bus_type,
            is_removable,
        });
    }
    devices.sort_by(|a, b| a.id.cmp(&b.id));
    devices
}

fn sysfs_attr(base: &Path, attr: &str) -> Option<String> {
    std::fs::read_to_string(base.join(attr))
        .ok()
        .map(|s| s.trim().to_string())
}

fn is_partition(base: &Path, name: &str) -> bool {
    if sysfs_attr(base, "partition").is_some() {
        return true;
    }
    if let Some(rest) = name.strip_prefix("mmcblk") {
        // whole device mmcblk0 vs partition mmcblk0p1 / boot mmcblk0boot0
        return rest.contains('p') || rest.contains("boot");
    }
    if let Some(rest) = name.strip_prefix("nvme") {
        // whole device nvme0n1 vs partition nvme0n1p1
        return rest.contains('p');
    }
    let last = name.chars().last();
    last.map(|c| c.is_ascii_digit()).unwrap_or(false)
}

fn sector_capacity(base: &Path) -> Option<u64> {
    let sectors: u64 = sysfs_attr(base, "size")?.parse().ok()?;
    Some(sectors * 512)
}

fn removable(base: &Path) -> bool {
    sysfs_attr(base, "removable")
        .map(|s| s == "1")
        .unwrap_or(false)
}

fn classify_media(name: &str, is_removable: bool) -> MediaType {
    if name.starts_with("sr") {
        return MediaType::CdRom;
    }
    if name.starts_with("nvme") {
        return MediaType::Nvme;
    }
    if name.starts_with("mmcblk") {
        return MediaType::SdFlash;
    }
    if name.starts_with("sd") {
        return if is_removable {
            MediaType::UsbFlash
        } else {
            MediaType::Hdd
        };
    }
    MediaType::Unknown
}

fn classify_bus(name: &str) -> BusType {
    if name.starts_with("nvme") {
        return BusType::Nvme;
    }
    if name.starts_with("mmcblk") {
        return BusType::Sd;
    }
    if name.starts_with("sr") {
        return BusType::Scsi;
    }
    if name.starts_with("sd") || name.starts_with("vd") || name.starts_with("hd") {
        return BusType::Sata;
    }
    BusType::Other(0)
}

fn model_and_serial(base: &Path, fallback_name: &str) -> (String, String) {
    let model = sysfs_attr(&base.join("device"), "model").unwrap_or_default();
    let serial = sysfs_attr(&base.join("device"), "serial").unwrap_or_default();
    if model.is_empty() {
        (fallback_name.to_string(), serial)
    } else {
        (model, serial)
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn linux_devices_are_sorted_with_unique_whole_disk_ids() {
        let devices = list_devices();
        let ids: Vec<&String> = devices.iter().map(|d| &d.id).collect();
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted, "ids must be sorted and unique");
        for d in &devices {
            assert!(d.id.starts_with("/dev/"), "id must be a block device path");
            assert!(d.capacity_bytes > 0, "capacity must be positive");
            assert!(!d.id.ends_with("1"), "partitions must be excluded");
        }
    }

    #[test]
    fn linux_media_classification_is_stable() {
        assert_eq!(classify_media("sda", false), MediaType::Hdd);
        assert_eq!(classify_media("sdb", true), MediaType::UsbFlash);
        assert_eq!(classify_media("nvme0n1", false), MediaType::Nvme);
        assert_eq!(classify_media("mmcblk0", false), MediaType::SdFlash);
        assert_eq!(classify_media("sr0", false), MediaType::CdRom);
        assert_eq!(classify_bus("nvme0n1"), BusType::Nvme);
        assert_eq!(classify_bus("sda"), BusType::Sata);
        assert_eq!(classify_bus("mmcblk0"), BusType::Sd);
    }

    #[test]
    fn linux_partition_detection_is_correct() {
        let base = std::env::temp_dir().join("purgent-no-sysfs");
        assert!(!is_partition(&base, "sda"));
        assert!(is_partition(&base, "sda1"));
        assert!(!is_partition(&base, "nvme0n1"));
        assert!(is_partition(&base, "nvme0n1p1"));
        assert!(!is_partition(&base, "mmcblk0"));
        assert!(is_partition(&base, "mmcblk0p1"));
        assert!(is_partition(&base, "mmcblk0boot0"));
    }
}
