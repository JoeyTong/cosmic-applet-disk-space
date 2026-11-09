//! Read-only storage inventory. lsblk supplies block topology and statvfs usage;
//! it runs in a cancellable child process, never on the UI thread.

use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const REFRESH_INTERVAL: Duration = Duration::from_secs(2);
pub const SCAN_TIMEOUT: Duration = Duration::from_secs(5);
pub const WARNING_PERCENT: u64 = 80;

const LSBLK_COLUMNS: &str = "NAME,KNAME,PATH,TYPE,TRAN,ROTA,RM,HOTPLUG,SIZE,FSTYPE,LABEL,UUID,MOUNTPOINTS,MODEL,MAJ:MIN,FSSIZE,FSUSED,FSAVAIL";

#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub sampled_at_unix_ms: u128,
    pub scan_duration_ms: u128,
    pub drives: Vec<Drive>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Drive {
    pub name: String,
    pub path: String,
    pub label: String,
    pub model: Option<String>,
    pub kind: DriveKind,
    pub raw_capacity_bytes: u64,
    pub usage: Option<Usage>,
    pub status: DriveStatus,
    pub shared: bool,
    pub filesystems: Vec<Filesystem>,
}

impl Drive {
    pub fn warning(&self) -> bool {
        self.usage.is_some_and(Usage::warning)
    }

    pub fn panel_summary(&self) -> String {
        let summary = self.usage.map_or_else(
            || self.status.label().to_owned(),
            |usage| format!("{} · {}", usage.panel_capacity(), usage.percent_label()),
        );
        format!(
            "{summary}{}{}",
            if self.shared { " · Shared" } else { "" },
            if self.status == DriveStatus::Partial {
                " · Partial"
            } else {
                ""
            }
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DriveKind {
    InternalSsd,
    InternalHdd,
    ExternalSsd,
    ExternalHdd,
    UsbFlash,
    SdCard,
    Unknown,
}

impl DriveKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::InternalSsd => "Internal SSD",
            Self::InternalHdd => "Internal HDD",
            Self::ExternalSsd => "External SSD",
            Self::ExternalHdd => "External HDD",
            Self::UsbFlash => "USB flash drive",
            Self::SdCard => "SD / memory card",
            Self::Unknown => "Local drive",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DriveStatus {
    Mounted,
    Partial,
    Unmounted,
    Locked,
    Unavailable,
    SwapOnly,
}

impl DriveStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Mounted => "Mounted",
            Self::Partial => "Partial usage",
            Self::Unmounted => "Unmounted",
            Self::Locked => "Locked",
            Self::Unavailable => "Unavailable",
            Self::SwapOnly => "Swap only",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Usage {
    pub used_bytes: u64,
    pub total_bytes: u64,
    pub available_bytes: Option<u64>,
}

impl Usage {
    /// Whole binary units, with one shared suffix to keep panel entries compact.
    pub fn panel_capacity(self) -> String {
        const UNITS: [&str; 7] = ["B", "K", "M", "G", "T", "P", "E"];
        let mut divisor = 1.0;
        let mut unit = 0;
        while self.total_bytes as f64 / divisor >= 1024.0 && unit < UNITS.len() - 1 {
            divisor *= 1024.0;
            unit += 1;
        }
        format!(
            "{:.0} / {:.0} {}",
            self.used_bytes as f64 / divisor,
            self.total_bytes as f64 / divisor,
            UNITS[unit]
        )
    }

    pub fn percent(self) -> f64 {
        if self.total_bytes == 0 {
            return 0.0;
        }
        self.used_bytes as f64 / self.total_bytes as f64 * 100.0
    }

    pub fn percent_label(self) -> String {
        // Truncate, so 79.99% never says 80% without a warning.
        let tenths = if self.total_bytes == 0 {
            0
        } else {
            u128::from(self.used_bytes) * 1000 / u128::from(self.total_bytes)
        };
        format!("{}.{:01}%", tenths / 10, tenths % 10)
    }

    pub fn warning(self) -> bool {
        self.total_bytes > 0
            && u128::from(self.used_bytes) * 100
                >= u128::from(self.total_bytes) * u128::from(WARNING_PERCENT)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Filesystem {
    pub devices: Vec<String>,
    pub label: Option<String>,
    pub filesystem_type: String,
    pub mountpoints: Vec<String>,
    pub usage: Option<Usage>,
    pub status: DriveStatus,
    /// All physical owners, including this drive. Multiple owners mean shared usage.
    pub physical_drives: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Inventory {
    blockdevices: Vec<BlockDevice>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct BlockDevice {
    name: String,
    kname: String,
    path: String,
    #[serde(rename = "type")]
    device_type: String,
    tran: Option<String>,
    #[serde(deserialize_with = "optional_bool")]
    rota: Option<bool>,
    #[serde(deserialize_with = "optional_bool")]
    rm: Option<bool>,
    #[serde(deserialize_with = "optional_bool")]
    hotplug: Option<bool>,
    #[serde(deserialize_with = "optional_u64")]
    size: Option<u64>,
    fstype: Option<String>,
    label: Option<String>,
    uuid: Option<String>,
    mountpoints: Vec<Option<String>>,
    model: Option<String>,
    #[serde(rename = "maj:min")]
    major_minor: String,
    #[serde(deserialize_with = "optional_u64")]
    fssize: Option<u64>,
    #[serde(deserialize_with = "optional_u64")]
    fsused: Option<u64>,
    #[serde(deserialize_with = "optional_u64")]
    fsavail: Option<u64>,
    children: Vec<BlockDevice>,
}

fn optional_u64<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<u64>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Number {
        Integer(u64),
        Text(String),
    }
    match Option::<Number>::deserialize(deserializer)? {
        Some(Number::Integer(value)) => Ok(Some(value)),
        Some(Number::Text(value)) if !value.is_empty() => {
            value.parse().map(Some).map_err(serde::de::Error::custom)
        }
        _ => Ok(None),
    }
}

fn optional_bool<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<bool>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Boolean {
        Bool(bool),
        Integer(u8),
        Text(String),
    }
    match Option::<Boolean>::deserialize(deserializer)? {
        Some(Boolean::Bool(value)) => Ok(Some(value)),
        Some(Boolean::Integer(value @ 0..=1)) => Ok(Some(value == 1)),
        Some(Boolean::Text(value)) if value == "0" || value == "1" => Ok(Some(value == "1")),
        Some(Boolean::Text(value)) if value.is_empty() => Ok(None),
        None => Ok(None),
        _ => Err(serde::de::Error::custom("expected a boolean or 0/1")),
    }
}

#[derive(Default)]
struct HardwareInfo {
    virtual_device: bool,
    remote: bool,
    properties: BTreeMap<String, String>,
    mmc_type: Option<String>,
}

fn inspect_hardware(device: &BlockDevice) -> HardwareInfo {
    let name = if device.kname.is_empty() {
        &device.name
    } else {
        &device.kname
    };
    let sysfs = Path::new("/sys/class/block").join(name);
    let properties: BTreeMap<_, _> =
        std::fs::read_to_string(format!("/run/udev/data/b{}", device.major_minor))
            .unwrap_or_default()
            .lines()
            .filter_map(|line| line.strip_prefix("E:")?.split_once('='))
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .collect();
    let remote = properties
        .get("ID_PATH")
        .is_some_and(|path| path.contains("iscsi"))
        || std::fs::read_to_string(sysfs.join("device/transport"))
            .is_ok_and(|transport| matches!(transport.trim(), "tcp" | "rdma" | "fc"));
    HardwareInfo {
        virtual_device: sysfs
            .canonicalize()
            .is_ok_and(|path| path.starts_with("/sys/devices/virtual")),
        remote,
        properties,
        mmc_type: std::fs::read_to_string(sysfs.join("device/type"))
            .ok()
            .map(|value| value.trim().to_owned()),
    }
}

fn classify(device: &BlockDevice, info: &HardwareInfo) -> DriveKind {
    let property = |key: &str| info.properties.get(key).is_some_and(|value| value == "1");
    let transport = device.tran.as_deref().unwrap_or_default();
    if info.mmc_type.as_deref() == Some("SD")
        || [
            "ID_DRIVE_FLASH_SD",
            "ID_DRIVE_FLASH_SDHC",
            "ID_DRIVE_FLASH_MMC",
            "ID_DRIVE_FLASH_CF",
            "ID_DRIVE_FLASH_MS",
        ]
        .iter()
        .any(|key| property(key))
        || (device.name.starts_with("mmcblk") && device.rm == Some(true))
    {
        return DriveKind::SdCard;
    }
    if transport == "usb"
        && device.rota != Some(true)
        && (property("ID_DRIVE_THUMB") || property("ID_DRIVE_FLASH") || device.rm == Some(true))
    {
        return DriveKind::UsbFlash;
    }
    let external = matches!(transport, "usb" | "ieee1394" | "firewire" | "thunderbolt")
        || device.rm == Some(true)
        || property("ID_DRIVE_EXTERNAL");
    match (external, device.rota) {
        (true, Some(true)) => DriveKind::ExternalHdd,
        (true, Some(false)) => DriveKind::ExternalSsd,
        (false, Some(true)) => DriveKind::InternalHdd,
        (false, Some(false)) => DriveKind::InternalSsd,
        (_, None) if transport == "nvme" => DriveKind::InternalSsd,
        _ => DriveKind::Unknown,
    }
}

fn local_disk(device: &BlockDevice) -> bool {
    device.device_type == "disk"
        && !["loop", "ram", "zram", "nbd", "rbd", "drbd", "zd"]
            .iter()
            .any(|prefix| device.name.starts_with(prefix))
        && !(device.name.starts_with("mmcblk")
            && (device.name.contains("boot") || device.name.contains("rpmb")))
        && !matches!(device.tran.as_deref(), Some("iscsi" | "fcoe"))
}

fn find_disks<'a>(nodes: &'a [BlockDevice], disks: &mut BTreeMap<String, Vec<&'a BlockDevice>>) {
    for node in nodes {
        if local_disk(node) {
            disks.entry(node.path.clone()).or_default().push(node);
        } else {
            find_disks(&node.children, disks);
        }
    }
}

fn filesystem_key(node: &BlockDevice) -> String {
    // Btrfs subvolumes and all members of a multi-device filesystem share an FSID.
    // Other filesystems use device identity: cloned UUIDs must not merge two disks.
    if node.fstype.as_deref() == Some("btrfs")
        && let Some(uuid) = node.uuid.as_ref().filter(|uuid| !uuid.is_empty())
    {
        return format!("btrfs:{uuid}");
    }
    if node.major_minor.is_empty() {
        node.path.clone()
    } else {
        node.major_minor.clone()
    }
}

fn filesystem(node: &BlockDevice) -> Option<Filesystem> {
    let mounts: BTreeSet<_> = node
        .mountpoints
        .iter()
        .flatten()
        .filter(|mount| mount.starts_with('/'))
        .cloned()
        .collect();
    let fstype = node.fstype.as_deref().unwrap_or_default();
    let container = matches!(
        fstype,
        "crypto_LUKS" | "LVM2_member" | "linux_raid_member" | "isw_raid_member"
    );
    if (container && !node.children.is_empty()) || (fstype.is_empty() && mounts.is_empty()) {
        return None;
    }
    let usage = if mounts.is_empty() {
        None
    } else {
        node.fssize.zip(node.fsused).and_then(|(total, used)| {
            (total > 0 && used <= total).then_some(Usage {
                used_bytes: used,
                total_bytes: total,
                available_bytes: node.fsavail,
            })
        })
    };
    let status = if usage.is_some() {
        DriveStatus::Mounted
    } else if !mounts.is_empty() || (container && fstype != "crypto_LUKS") || fstype == "zfs_member"
    {
        DriveStatus::Unavailable
    } else if fstype == "crypto_LUKS" {
        DriveStatus::Locked
    } else if fstype == "swap" {
        DriveStatus::SwapOnly
    } else {
        DriveStatus::Unmounted
    };
    Some(Filesystem {
        devices: vec![node.path.clone()],
        label: clean_optional(&node.label),
        filesystem_type: if fstype.is_empty() { "unknown" } else { fstype }.to_owned(),
        mountpoints: mounts.into_iter().collect(),
        usage,
        status,
        physical_drives: Vec::new(),
    })
}

fn collect_filesystems(
    node: &BlockDevice,
    owner: &str,
    keys: &mut BTreeSet<String>,
    filesystems: &mut BTreeMap<String, Filesystem>,
) {
    if let Some(mut new) = filesystem(node) {
        let key = filesystem_key(node);
        keys.insert(key.clone());
        new.physical_drives.push(owner.to_owned());
        if let Some(existing) = filesystems.get_mut(&key) {
            if existing.usage.is_none() && new.usage.is_some() {
                existing.usage = new.usage;
                existing.status = DriveStatus::Mounted;
            } else if existing.usage.is_none() && !new.mountpoints.is_empty() {
                existing.status = DriveStatus::Unavailable;
            }
            if existing.label.is_none() {
                existing.label = new.label;
            }
            merge_unique(&mut existing.mountpoints, new.mountpoints);
            merge_unique(&mut existing.devices, new.devices);
            merge_unique(&mut existing.physical_drives, new.physical_drives);
        } else {
            filesystems.insert(key, new);
        }
    }
    for child in &node.children {
        collect_filesystems(child, owner, keys, filesystems);
    }
}

fn merge_unique(into: &mut Vec<String>, other: Vec<String>) {
    into.extend(other);
    into.sort();
    into.dedup();
}

fn clean_optional(value: &Option<String>) -> Option<String> {
    value.as_ref().and_then(|value| {
        let clean: String = value
            .chars()
            .filter(|character| !character.is_control())
            .collect();
        let clean = clean.trim();
        (!clean.is_empty()).then(|| clean.to_owned())
    })
}

fn aggregate(filesystems: &[Filesystem]) -> (Option<Usage>, DriveStatus) {
    let mut combined: Option<Usage> = None;
    for usage in filesystems.iter().filter_map(|filesystem| filesystem.usage) {
        combined = Some(combined.map_or(usage, |total| {
            Usage {
                used_bytes: total.used_bytes.saturating_add(usage.used_bytes),
                total_bytes: total.total_bytes.saturating_add(usage.total_bytes),
                available_bytes: total
                    .available_bytes
                    .zip(usage.available_bytes)
                    .map(|(left, right)| left.saturating_add(right)),
            }
        }));
    }
    let has = |status| {
        filesystems
            .iter()
            .any(|filesystem| filesystem.status == status)
    };
    let status = if combined.is_some() {
        if has(DriveStatus::Unavailable) {
            DriveStatus::Partial
        } else {
            DriveStatus::Mounted
        }
    } else if has(DriveStatus::Unavailable) {
        DriveStatus::Unavailable
    } else if has(DriveStatus::Locked) {
        DriveStatus::Locked
    } else if !filesystems.is_empty()
        && filesystems
            .iter()
            .all(|fs| fs.status == DriveStatus::SwapOnly)
    {
        DriveStatus::SwapOnly
    } else {
        DriveStatus::Unmounted
    };
    (combined, status)
}

fn parse_inventory(
    json: &[u8],
    inspect: impl Fn(&BlockDevice) -> HardwareInfo,
) -> Result<Vec<Drive>, String> {
    let inventory: Inventory =
        serde_json::from_slice(json).map_err(|error| format!("Invalid lsblk output: {error}"))?;
    let mut nodes = BTreeMap::new();
    find_disks(&inventory.blockdevices, &mut nodes);
    let mut filesystems = BTreeMap::new();
    let mut pending = Vec::new();
    for variants in nodes.values() {
        let device = variants[0];
        let hardware = inspect(device);
        if hardware.virtual_device || hardware.remote {
            continue;
        }
        let mut keys = BTreeSet::new();
        for variant in variants {
            collect_filesystems(variant, &device.path, &mut keys, &mut filesystems);
        }
        pending.push((device, classify(device, &hardware), keys));
    }
    let mut drives = Vec::new();
    for (device, kind, keys) in pending {
        let filesystems: Vec<_> = keys
            .iter()
            .filter_map(|key| filesystems.get(key).cloned())
            .collect();
        let (usage, status) = aggregate(&filesystems);
        let label = clean_optional(&device.label)
            .or_else(|| {
                filesystems
                    .iter()
                    .find_map(|filesystem| filesystem.label.clone())
            })
            .unwrap_or_else(|| device.name.clone());
        drives.push(Drive {
            name: device.name.clone(),
            path: device.path.clone(),
            label,
            model: clean_optional(&device.model),
            kind,
            raw_capacity_bytes: device.size.unwrap_or_default(),
            usage,
            status,
            shared: filesystems
                .iter()
                .any(|filesystem| filesystem.physical_drives.len() > 1),
            filesystems,
        });
    }
    Ok(drives)
}

pub async fn scan() -> Result<Snapshot, String> {
    let started = Instant::now();
    let mut command = tokio::process::Command::new("lsblk");
    command
        .args(["--json", "--bytes", "--tree", "--output", LSBLK_COLUMNS])
        .env("LC_ALL", "C")
        .kill_on_drop(true);
    let output = tokio::time::timeout(SCAN_TIMEOUT, command.output())
        .await
        .map_err(|_| "Storage scan timed out after 5 seconds; retrying automatically".to_owned())?
        .map_err(|error| format!("Cannot run lsblk (install util-linux): {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "lsblk failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let drives =
        tokio::task::spawn_blocking(move || parse_inventory(&output.stdout, inspect_hardware))
            .await
            .map_err(|error| format!("Storage worker failed: {error}"))??;
    Ok(Snapshot {
        sampled_at_unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
        scan_duration_ms: started.elapsed().as_millis(),
        drives,
    })
}

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 7] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];
    let mut value = bytes as f64;
    let mut index = 0;
    while value >= 1024.0 && index < UNITS.len() - 1 {
        value /= 1024.0;
        index += 1;
    }
    if index == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[index])
    }
}

#[cfg(test)]
#[path = "storage_tests.rs"]
mod tests;
